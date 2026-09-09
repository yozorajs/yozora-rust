use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::document::MAX_DOCUMENT_BYTES;
use crate::{cancellation::Cancellation, protocol::ResponseError};

pub const MAX_WORKSPACE_ROOTS: usize = 128;
pub const MAX_WORKSPACE_ENTRIES: usize = 20_000;
pub const MAX_WORKSPACE_FILES: usize = 2_000;
pub const MAX_WORKSPACE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Eq, PartialEq)]
pub struct SourceBuffer {
    pub uri: String,
    pub path: Option<PathBuf>,
    pub ambiguous: bool,
}

pub struct Sources {
    pub files: Vec<PathBuf>,
    pub buffers: Vec<SourceBuffer>,
}

/// URI roots belong to the server. Resolve paths afresh for each query so moved
/// files and symlinks cannot leave a stale disk cache or access boundary behind.
#[derive(Clone, Default)]
pub struct Workspace {
    folders: Vec<String>,
}

impl Workspace {
    pub fn new(folders: Vec<String>) -> Self {
        Self { folders }
    }

    pub fn change(&mut self, removed: &[String], added: Vec<String>) {
        self.folders.retain(|uri| !removed.contains(uri));
        for uri in added {
            if !self.folders.contains(&uri) {
                self.folders.push(uri);
            }
        }
    }

    pub fn scope(&self, source_uri: &str) -> FileScope {
        if self.folders.is_empty() {
            FileScope::new(
                file_uri_path(source_uri)
                    .and_then(|path| path.parent().map(Path::to_path_buf))
                    .into_iter(),
            )
        } else {
            FileScope::new(self.folders.iter().filter_map(|uri| file_uri_path(uri)))
        }
    }

    /// Indexing only visits explicit local roots. Neither an absent workspace
    /// nor a remote root grants traversal of the cwd or an open file's parent.
    pub fn index_scope(
        &self,
        cancellation: &Cancellation,
        max_roots: usize,
    ) -> Result<FileScope, ResponseError> {
        cancellation.check()?;
        if self.folders.len() > max_roots {
            return Err(ResponseError::new(
                -32000,
                "workspace index exceeds the root limit; use fewer workspace roots",
            ));
        }
        let scope = FileScope::new(
            self.folders
                .iter()
                .take_while(|_| !cancellation.is_cancelled())
                .filter_map(|uri| file_uri_path(uri)),
        );
        cancellation.check()?;
        Ok(scope)
    }
}

#[derive(Eq, PartialEq)]
pub struct FileScope {
    roots: Vec<(PathBuf, PathBuf)>,
}

impl FileScope {
    pub fn open_file_uris<'a>(
        &self,
        uris: impl Iterator<Item = &'a str>,
        cancellation: &Cancellation,
    ) -> Result<HashMap<PathBuf, Vec<String>>, ResponseError> {
        cancellation.check()?;
        let mut uris: Vec<_> = uris.collect();
        uris.sort_unstable();
        let mut result: HashMap<PathBuf, Vec<String>> = HashMap::new();
        for uri in uris {
            cancellation.check()?;
            if let Some(path) = file_uri_path(uri).and_then(|path| self.resolve(&path)) {
                result.entry(path).or_default().push(uri.to_string());
            }
        }
        Ok(result)
    }

    fn new(roots: impl Iterator<Item = PathBuf>) -> Self {
        Self {
            roots: roots
                .filter_map(|path| canonical_path(&path).map(|canonical| (path, canonical)))
                .collect(),
        }
    }

    pub fn resolve(&self, path: &Path) -> Option<PathBuf> {
        let path = normalize_path(path)?;
        // Check spelling before any filesystem access, then check the resolved
        // path again. Unsaved files use their nearest existing ancestor.
        if blocked_path(&path)
            || !self
                .roots
                .iter()
                .any(|(root, canonical)| path.starts_with(root) || path.starts_with(canonical))
        {
            return None;
        }
        let canonical = canonical_path(&path)?;
        self.roots
            .iter()
            .any(|(_, root)| canonical.starts_with(root))
            .then_some(canonical)
    }

    fn index_excluded(&self, path: &Path) -> bool {
        // Generated trees are excluded relative to a declared root. A project
        // explicitly rooted inside `.cache` or `target` can still be indexed.
        blocked_path(path)
            || !self.roots.iter().any(|(lexical, canonical)| {
                [lexical, canonical].into_iter().any(|root| {
                    path.strip_prefix(root).is_ok_and(|relative| {
                        !relative.components().any(|component| {
                            matches!(
                                component.as_os_str().to_str(),
                                Some("target" | "node_modules" | ".cache" | ".venv")
                            )
                        })
                    })
                })
            })
    }

    /// Discover disk files and overlay open buffers once. Read queries select a
    /// stable alias; refactors can reject ambiguous buffers before offering edits.
    pub fn sources<'a>(
        &self,
        uris: impl Iterator<Item = &'a str>,
        cancellation: &Cancellation,
        max_entries: usize,
        max_files: usize,
    ) -> Result<Sources, ResponseError> {
        let mut files = self.markdown_files(cancellation, max_entries, max_files)?;
        let mut opened: BTreeMap<PathBuf, (u8, String, bool)> = BTreeMap::new();
        let mut buffers = Vec::new();
        for uri in uris {
            cancellation.check()?;
            if let Some(lexical) = file_uri_path(uri) {
                let path = self.resolve(&lexical).unwrap_or_else(|| lexical.clone());
                let rank = if path_uri(&path).as_deref() == Some(uri) {
                    0
                } else if lexical == path {
                    1
                } else {
                    2
                };
                opened
                    .entry(path)
                    .and_modify(|selected| {
                        selected.2 = true;
                        if (rank, uri) < (selected.0, selected.1.as_str()) {
                            selected.0 = rank;
                            selected.1 = uri.to_string();
                        }
                    })
                    .or_insert((rank, uri.to_string(), false));
            } else if !uri
                .split_once(':')
                .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
            {
                buffers.push(SourceBuffer {
                    uri: uri.to_string(),
                    path: None,
                    ambiguous: false,
                });
            }
            if opened.len() + buffers.len() > max_files {
                return Err(ResponseError::new(
                    -32000,
                    "workspace exceeds the file limit; use narrower roots or close buffers",
                ));
            }
        }
        files.retain(|path| !opened.contains_key(path));
        buffers.extend(
            opened
                .into_iter()
                .map(|(path, (_, uri, ambiguous))| SourceBuffer {
                    uri,
                    path: Some(path),
                    ambiguous,
                }),
        );
        buffers.sort_by(|left, right| left.uri.cmp(&right.uri));
        if files.len() + buffers.len() > max_files {
            return Err(ResponseError::new(
                -32000,
                "workspace exceeds the file limit; use narrower roots or close buffers",
            ));
        }
        Ok(Sources { files, buffers })
    }

    /// Return canonical, unique Markdown paths in a stable order. Count every
    /// directory entry, including skipped names, so wide trees remain bounded.
    /// Limits reject the whole query instead of silently reporting a partial index.
    pub fn markdown_files(
        &self,
        cancellation: &Cancellation,
        max_entries: usize,
        max_files: usize,
    ) -> Result<Vec<PathBuf>, ResponseError> {
        cancellation.check()?;
        let mut directories: Vec<_> = self.roots.iter().map(|(_, path)| path.clone()).collect();
        let mut visited = BTreeSet::new();
        let mut files = BTreeSet::new();
        let mut inspected = 0;
        while let Some(directory) = directories.pop() {
            cancellation.check()?;
            if self.index_excluded(&directory) || !visited.insert(directory.clone()) {
                continue;
            }
            let Some(directory) = self.resolve(&directory) else {
                continue;
            };
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => {
                    return Err(ResponseError::new(
                        -32000,
                        "cannot read a workspace directory",
                    ))
                }
            };
            for entry in entries {
                cancellation.check()?;
                if inspected == max_entries {
                    return Err(ResponseError::new(-32000, "workspace index exceeds the directory entry limit; use narrower workspace roots"));
                }
                inspected += 1;
                let entry = entry.map_err(|_| {
                    ResponseError::new(-32000, "cannot read a workspace directory entry")
                })?;
                if self.index_excluded(&entry.path()) {
                    continue;
                }
                let Some(path) = self.resolve(&entry.path()) else {
                    continue;
                };
                if self.index_excluded(&path) {
                    continue;
                }
                let Ok(metadata) = path.metadata() else {
                    continue;
                };
                if metadata.is_dir() {
                    directories.push(path);
                } else if metadata.is_file() && is_markdown(&path) {
                    files.insert(path);
                    if files.len() > max_files {
                        return Err(ResponseError::new(
                            -32000,
                            "workspace index exceeds the file limit; use narrower workspace roots",
                        ));
                    }
                }
            }
        }
        Ok(files.into_iter().collect())
    }
}

pub struct LocalFile {
    // Keep the requested URI spelling for buffer selection. Only the canonical,
    // scope-checked path may be used for filesystem access.
    pub uri: String,
    pub path: PathBuf,
}

pub fn open_file_uri<'a>(
    open_uris: &'a HashMap<PathBuf, Vec<String>>,
    file: &LocalFile,
    source_uri: &str,
) -> Option<&'a str> {
    let aliases = open_uris.get(&file.path)?;
    // Buffer identity is the requested URI, then its normalized lexical path.
    // Source preference belongs to the lookup, so a workspace scan can reuse
    // the same alias map for resources in different source buffers.
    let lexical = file_uri_path(&file.uri);
    aliases
        .iter()
        .min_by_key(|uri| {
            (
                *uri != &file.uri,
                lexical.is_none() || file_uri_path(uri) != lexical,
                uri.as_str() != source_uri,
                uri.as_str(),
            )
        })
        .map(String::as_str)
}

pub enum Target {
    External(String),
    Local {
        // None means the current buffer, including non-file document URIs.
        file: Option<LocalFile>,
        query: Option<String>,
        fragment: Option<String>,
    },
}

pub fn resolve(source_uri: &str, destination: &str, scope: &FileScope) -> Option<Target> {
    if destination.chars().any(char::is_control) || destination.contains('\\') {
        return None;
    }
    let (resource, fragment) = split_suffix(destination, '#');
    let (resource, query) = split_suffix(resource, '?');
    if let Some((scheme, rest)) = resource.split_once(':') {
        if valid_scheme(scheme) {
            if matches!(
                scheme.to_ascii_lowercase().as_str(),
                "http" | "https" | "mailto"
            ) {
                if rest.is_empty()
                    || (!scheme.eq_ignore_ascii_case("mailto")
                        && !rest.strip_prefix("//").is_some_and(|value| {
                            !value.split('/').next().unwrap_or_default().is_empty()
                        }))
                {
                    return None;
                }
                return Some(Target::External(encode_uri(destination)?));
            }
            if !scheme.eq_ignore_ascii_case("file") {
                return None;
            }
        }
    }
    let fragment = match fragment {
        Some(fragment) => Some(percent_decode(fragment)?),
        None => None,
    };
    let query = match query {
        Some(query) => Some(encode_uri(query)?),
        None => None,
    };
    let file = if resource.is_empty() {
        None
    } else {
        let decoded = percent_decode(resource)?;
        // A trailing slash or dot segment denotes a directory. Normalizing it
        // away would incorrectly turn `guide.md/` into a file destination.
        if decoded
            .rsplit('/')
            .next()
            .is_some_and(|part| matches!(part, "" | "." | ".."))
        {
            return None;
        }
        let lexical = normalize_path(&local_path(source_uri, resource)?)?;
        let path = scope.resolve(&lexical)?;
        let uri = if resource
            .split_once(':')
            .is_some_and(|(scheme, _)| valid_scheme(scheme))
        {
            resource.to_string()
        } else {
            path_uri(&lexical)?
        };
        Some(LocalFile { uri, path })
    };
    Some(Target::Local {
        file,
        query,
        fragment,
    })
}

fn local_path(source_uri: &str, resource: &str) -> Option<PathBuf> {
    if resource
        .split_once(':')
        .is_some_and(|(scheme, _)| valid_scheme(scheme))
    {
        return file_uri_path(resource);
    }
    let decoded = percent_decode(resource)?;
    if decoded.contains('\\') || decoded.starts_with("//") {
        return None;
    }
    if Path::new(&decoded).is_absolute() {
        return Some(PathBuf::from(decoded));
    }
    let source = file_uri_path(source_uri)?;
    Some(source.parent()?.join(decoded))
}

pub struct PathCandidate {
    pub name: String,
    pub directory: bool,
}

/// The first editable path position excludes a URI's scheme, authority and
/// absolute root. Leave path escapes undecoded so partial prefixes remain valid.
pub fn completion_path_start(resource: &str) -> Option<usize> {
    if resource.contains('\\') || resource.chars().any(char::is_control) {
        return None;
    }
    let path = if let Some((scheme, rest)) = resource
        .split_once(':')
        .filter(|(scheme, _)| valid_scheme(scheme))
    {
        if !scheme.eq_ignore_ascii_case("file") {
            return None;
        }
        let path = if let Some(authority_and_path) = rest.strip_prefix("//") {
            let (authority, _) = authority_and_path.split_once('/')?;
            if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
                return None;
            }
            &authority_and_path[authority.len()..]
        } else {
            rest
        };
        if !path.starts_with('/') {
            return None;
        }
        path
    } else {
        resource
    };
    let mut separators = path_separators(path);
    let root_end = separators
        .next()
        .filter(|separator| separator.start == 0)
        .map_or(0, |separator| separator.end);
    if root_end > 0
        && separators
            .next()
            .is_some_and(|separator| separator.start == root_end)
    {
        return None;
    }
    Some(resource.len() - path.len() + root_end)
}

/// List one directory, merging unsaved descendants supplied by the server.
/// This performs at most 1,000 directory-entry inspections and returns at most
/// 200 names. No traversal, disk cache, or reads of file contents are involved.
pub fn path_candidates<'a>(
    source_uri: &str,
    prefix: &str,
    scope: &FileScope,
    open_paths: impl Iterator<Item = &'a Path>,
) -> Vec<PathCandidate> {
    if prefix.contains(['?', '#']) || completion_path_start(prefix).is_none() {
        return Vec::new();
    }
    let (parent, prefix) = path_separators(prefix)
        .next_back()
        .map_or(("", prefix), |separator| {
            (&prefix[..separator.end], &prefix[separator.end..])
        });
    let Some(prefix) = component_prefix(prefix) else {
        return Vec::new();
    };
    let Some(parent) = local_path(source_uri, parent).and_then(|path| scope.resolve(&path)) else {
        return Vec::new();
    };
    let mut candidates = BTreeMap::new();
    let mut insert = |name: String, directory: bool| {
        candidates
            .entry(name)
            .and_modify(|value| *value |= directory)
            .or_insert(directory);
        if candidates.len() > 200 {
            candidates.pop_last();
        }
    };
    if let Ok(entries) = fs::read_dir(&parent) {
        for entry in entries.take(1_000).filter_map(Result::ok) {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !encode_component(&name).starts_with(&prefix) {
                continue;
            }
            let Some(path) = scope.resolve(&entry.path()) else {
                continue;
            };
            let Ok(metadata) = path.metadata() else {
                continue;
            };
            if metadata.is_file() || metadata.is_dir() {
                insert(name, metadata.is_dir());
            }
        }
    }
    for path in open_paths {
        let Ok(relative) = path.strip_prefix(&parent) else {
            continue;
        };
        let mut components = relative.components();
        let Some(Component::Normal(name)) = components.next() else {
            continue;
        };
        let Some(name) = name.to_str() else { continue };
        if encode_component(name).starts_with(&prefix) && !blocked_path(relative) {
            insert(name.to_string(), components.next().is_some());
        }
    }
    candidates
        .into_iter()
        .map(|(name, directory)| PathCandidate { name, directory })
        .collect()
}

pub fn encode_component(value: &str) -> String {
    percent_encode(value.as_bytes(), false)
}

pub fn path_separators(
    value: &str,
) -> impl DoubleEndedIterator<Item = std::ops::Range<usize>> + '_ {
    let bytes = value.as_bytes();
    bytes.iter().enumerate().filter_map(move |(index, &byte)| {
        if byte == b'/' {
            Some(index..index + 1)
        } else if bytes
            .get(index..index + 3)
            .is_some_and(|escape| escape.eq_ignore_ascii_case(b"%2f"))
        {
            Some(index..index + 3)
        } else {
            None
        }
    })
}

pub fn component_prefix(prefix: &str) -> Option<String> {
    // Canonicalize once per request, including partial percent escapes and
    // partial UTF-8 sequences. Raw characters and encoded spellings can mix.
    let mut bytes = prefix.bytes();
    let mut decoded = Vec::new();
    let mut suffix = String::new();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            decoded.push(byte);
            continue;
        }
        let Some(high) = bytes.next() else {
            suffix.push('%');
            break;
        };
        let high = hex(high)?;
        let Some(low) = bytes.next() else {
            suffix.push('%');
            suffix.push(b"0123456789ABCDEF"[high as usize] as char);
            break;
        };
        decoded.push((high << 4) | hex(low)?);
    }
    let mut normalized = percent_encode(&decoded, false);
    normalized.push_str(&suffix);
    Some(normalized)
}

fn split_suffix(value: &str, separator: char) -> (&str, Option<&str>) {
    value
        .split_once(separator)
        .map_or((value, None), |(head, tail)| (head, Some(tail)))
}

pub fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let (scheme, resource) = uri.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    let (resource, _) = split_suffix(resource, '#');
    let (resource, _) = split_suffix(resource, '?');
    let path = if let Some(authority_and_path) = resource.strip_prefix("//") {
        let (authority, path) = authority_and_path.split_once('/')?;
        if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
            return None;
        }
        format!("/{path}")
    } else {
        resource.to_string()
    };
    if !path.starts_with('/') || path.starts_with("//") {
        return None;
    }
    let path = percent_decode(&path)?;
    if path.starts_with("//") || path.contains('\\') {
        return None;
    }
    #[cfg(windows)]
    let path = path.strip_prefix('/').unwrap_or(&path);
    let path = normalize_path(Path::new(&path))?;
    (!blocked_path(&path)).then_some(path)
}

pub fn path_uri(path: &Path) -> Option<String> {
    let path = path.to_str()?;
    #[cfg(windows)]
    let path = format!("/{}", path.replace('\\', "/"));
    Some(format!("file://{}", percent_encode(path.as_bytes(), true)))
}

pub fn link_uri(uri: &str, query: Option<&str>, fragment: Option<&str>) -> Option<String> {
    let (resource, _) = split_suffix(uri, '#');
    let resource = if query.is_some() {
        split_suffix(resource, '?').0
    } else {
        resource
    };
    let mut result = encode_uri(resource)?;
    if let Some(query) = query {
        result.push('?');
        result.push_str(query);
    }
    if let Some(fragment) = fragment {
        result.push('#');
        result.push_str(&percent_encode(fragment.as_bytes(), false));
    }
    Some(result)
}

pub fn is_file(path: &Path) -> bool {
    path.metadata().is_ok_and(|metadata| metadata.is_file())
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["md", "markdown", "mdown", "mkd", "mkdn", "yozora"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

pub fn read_markdown(path: &Path) -> Option<String> {
    let mut remaining = MAX_DOCUMENT_BYTES;
    read_markdown_budgeted(path, &mut remaining).ok().flatten()
}

/// Charge attempted reads, including invalid UTF-8 and partially failed reads.
/// Metadata-only exclusions do not consume the byte budget. The extra byte
/// detects growth past a limit between metadata inspection and the read.
pub fn read_markdown_budgeted(
    path: &Path,
    remaining: &mut usize,
) -> Result<Option<String>, ResponseError> {
    if !is_markdown(path) || blocked_path(path) {
        return Ok(None);
    }
    let Ok(metadata) = path.metadata() else {
        return Ok(None);
    };
    if !metadata.is_file() || metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Ok(None);
    }
    let Ok(file) = File::open(path) else {
        return Ok(None);
    };
    let Ok(metadata) = file.metadata() else {
        return Ok(None);
    };
    if !metadata.is_file() || metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Ok(None);
    }
    let exhausted = || {
        ResponseError::new(-32000, "workspace index exceeds the text byte limit; use narrower workspace roots or close buffers")
    };
    if metadata.len() > *remaining as u64 {
        return Err(exhausted());
    }
    let mut bytes = Vec::new();
    let read = file
        .take((MAX_DOCUMENT_BYTES.min(*remaining) + 1) as u64)
        .read_to_end(&mut bytes);
    if bytes.len() > *remaining {
        *remaining = 0;
        return Err(exhausted());
    }
    *remaining -= bytes.len();
    if read.is_err() || bytes.len() > MAX_DOCUMENT_BYTES {
        return Ok(None);
    }
    Ok(String::from_utf8(bytes).ok())
}

fn normalize_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    Some(result)
}

fn canonical_path(path: &Path) -> Option<PathBuf> {
    if blocked_path(path) {
        return None;
    }
    let mut ancestor = path;
    let mut missing = Vec::new();
    loop {
        match ancestor.canonicalize() {
            Ok(canonical) => {
                #[cfg(windows)]
                let canonical = PathBuf::from(
                    canonical
                        .to_str()?
                        .strip_prefix(r"\\?\")
                        .unwrap_or(canonical.to_str()?),
                );
                let mut canonical = canonical;
                for component in missing.into_iter().rev() {
                    canonical.push(component);
                }
                return (!blocked_path(&canonical)).then_some(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if ancestor
                    .symlink_metadata()
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    return None;
                }
                missing.push(ancestor.file_name()?);
                ancestor = ancestor.parent()?;
            }
            Err(_) => return None,
        }
    }
}

fn blocked_path(path: &Path) -> bool {
    let mut previous = String::new();
    for component in path.components() {
        let Some(value) = component.as_os_str().to_str() else {
            return true;
        };
        // Canonical targets and directory entries must also be representable by
        // the same file URI policy as links and open buffers.
        if matches!(component, Component::Normal(_))
            && (value.contains('\\') || value.chars().any(char::is_control))
        {
            return true;
        }
        let value = value.to_ascii_lowercase();
        if matches!(
            value.as_str(),
            ".ssh" | ".git" | ".git-credentials" | ".gnupg" | ".aws" | ".kube"
        ) || value.starts_with(".env")
            || value.ends_with(".http_request")
            || value.ends_with(".http_response")
            || (previous == "local" && value.starts_with("env."))
        {
            return true;
        }
        previous = value;
    }
    false
}

fn valid_scheme(scheme: &str) -> bool {
    scheme
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = value.bytes();
    let mut decoded = Vec::with_capacity(value.len());
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            (hex(bytes.next()?)? << 4) | hex(bytes.next()?)?
        } else {
            byte
        });
    }
    let decoded = String::from_utf8(decoded).ok()?;
    (!decoded.chars().any(char::is_control)).then_some(decoded)
}

fn hex(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|value| value as u8)
}

fn percent_encode(bytes: &[u8], path: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::new();
    for &byte in bytes {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
            || (path && matches!(byte, b'/' | b':'))
        {
            result.push(byte as char);
        } else {
            result.push('%');
            result.push(HEX[(byte >> 4) as usize] as char);
            result.push(HEX[(byte & 15) as usize] as char);
        }
    }
    result
}

pub fn encode_uri(uri: &str) -> Option<String> {
    // Preserve URI delimiters and existing escapes, encoding only raw Unicode
    // and characters which cannot occur in a URI. Never double-decode `%25`.
    let mut result = String::new();
    let mut bytes = uri.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next()?;
            let low = bytes.next()?;
            hex(high)?;
            hex(low)?;
            result.extend(['%', high as char, low as char]);
        } else if byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=".contains(&byte) {
            result.push(byte as char);
        } else if byte.is_ascii_control() || byte == b'\\' {
            return None;
        } else {
            result.push_str(&percent_encode(&[byte], false));
        }
    }
    Some(result)
}

#[cfg(test)]
pub(super) mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    pub struct TestDir(pub PathBuf);

    impl TestDir {
        pub fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            loop {
                let path = std::env::temp_dir().join(format!(
                    "yozora-lsp-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path.canonicalize().unwrap()),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("cannot create test directory: {error}"),
                }
            }
        }

        pub fn uri(&self, path: &str) -> String {
            path_uri(&self.0.join(path)).unwrap()
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn decodes_paths_once_without_treating_plus_or_encoded_delimiters_as_syntax() {
        let directory = TestDir::new();
        let source = directory.uri("source.md");
        let scope = Workspace::default().scope(&source);
        for (destination, filename, fragment) in [
            ("a%20b.md#intro", "a b.md", "intro"),
            (
                "%E4%B8%AD%E6%96%87.md#%E6%A0%87%E9%A2%98",
                "中文.md",
                "标题",
            ),
            ("a%23b%3Fc.md?q=/elsewhere#x%23y", "a#b?c.md", "x#y"),
            ("a%2523b.md#x%2523y", "a%23b.md", "x%23y"),
            ("a+b.md#x+y", "a+b.md", "x+y"),
        ] {
            let Some(Target::Local {
                file: Some(file),
                fragment: actual,
                ..
            }) = resolve(&source, destination, &scope)
            else {
                panic!("failed to resolve {destination}");
            };
            assert_eq!(file.path, directory.0.join(filename));
            assert_eq!(actual.as_deref(), Some(fragment));
            assert_eq!(file_uri_path(&file.uri), Some(file.path));
        }
        let encoded = directory.uri("a #?%+中文.md");
        assert!(encoded.ends_with("/a%20%23%3F%25%2B%E4%B8%AD%E6%96%87.md"));
        let alias = encoded
            .replacen("file://", "FILE://localhost", 1)
            .replace("%3F", "%3f");
        assert_eq!(file_uri_path(&alias), file_uri_path(&encoded));
        assert_eq!(
            link_uri(&source, Some("view=raw"), Some("标题+%#")),
            Some(format!("{source}?view=raw#%E6%A0%87%E9%A2%98%2B%25%23"))
        );
    }

    #[test]
    fn rejects_invalid_uris_and_execution_schemes_without_filesystem_access() {
        let scope = FileScope { roots: Vec::new() };
        for uri in [
            "file://host/docs/a.md",
            "file:relative.md",
            "file:///docs/%",
            "file:///docs/%0",
            "file:///docs/%GG",
            "file:///docs/%FF",
            "file:///docs/%00.md",
            "file:///docs/a%5Cb.md",
            "file:////host/a.md",
            "file:///%2Fhost/a.md",
        ] {
            assert!(file_uri_path(uri).is_none(), "{uri}");
        }
        for destination in [
            "javascript:alert(1)",
            "command:run",
            "data:text/plain,test",
            "ftp://host/file",
            "//host/file",
            "https://",
            "https:relative",
            "mailto:",
            "#%FF",
            "#%00",
            "#%G1",
        ] {
            assert!(
                resolve("untitled:source", destination, &scope).is_none(),
                "{destination}"
            );
        }
        assert!(matches!(
            resolve("untitled:source", "#title", &scope),
            Some(Target::Local { file: None, .. })
        ));
        assert!(resolve("untitled:source", "relative.md", &scope).is_none());
        assert!(
            matches!(resolve("untitled:source", "https://example.test/中文?q=a+b#top", &scope),
            Some(Target::External(uri)) if uri == "https://example.test/%E4%B8%AD%E6%96%87?q=a+b#top")
        );
        assert!(matches!(
            resolve("untitled:source", "mailto:a@example.test", &scope),
            Some(Target::External(_))
        ));
    }

    #[test]
    fn confines_paths_to_workspace_or_source_parent_without_guessing_missing_files() {
        let directory = TestDir::new();
        fs::create_dir(directory.0.join("docs")).unwrap();
        let source = directory.uri("docs/source.md");
        let fallback = Workspace::default().scope(&source);
        assert!(resolve(&source, "new/sub/guide.md", &fallback).is_some());
        assert!(resolve(&source, "../guide.md", &fallback).is_none());
        assert!(resolve(&source, "%2E%2E/guide.md", &fallback).is_none());
        let scope = Workspace::new(vec![directory.uri("")]).scope(&source);
        assert!(resolve(&source, "../guide.md", &scope).is_some());
        assert!(resolve(&source, "../../guide.md", &scope).is_none());
        let remote = Workspace::new(vec!["vscode-remote://host/docs".to_string()]).scope(&source);
        assert!(resolve(&source, "guide.md", &remote).is_none());
        assert!(resolve(&source, "#title", &remote).is_some());
    }

    #[test]
    fn absolute_destinations_from_untitled_buffers_stay_inside_explicit_roots() {
        let directory = TestDir::new();
        let uri = directory.uri("guide.md");
        let scope = Workspace::new(vec![directory.uri("")]).scope("untitled:source");
        assert!(matches!(
            resolve("untitled:source", &uri, &scope),
            Some(Target::Local { file: Some(_), .. })
        ));
        assert!(matches!(
            resolve(
                "untitled:source",
                directory.0.join("guide.md").to_str().unwrap(),
                &scope
            ),
            Some(Target::Local { file: Some(_), .. })
        ));
        assert!(resolve("untitled:source", "guide.md", &scope).is_none());
        assert!(resolve("untitled:source", "file:///outside/guide.md", &scope).is_none());
        let unscoped = Workspace::default().scope("untitled:source");
        assert!(resolve("untitled:source", &uri, &unscoped).is_none());
    }

    #[test]
    fn does_not_turn_directory_destinations_into_regular_files() {
        let directory = TestDir::new();
        fs::write(directory.0.join("guide.md"), "# Guide").unwrap();
        let source = directory.uri("source.md");
        let scope = Workspace::default().scope(&source);
        assert!(resolve(&source, "guide.md", &scope).is_some());
        for relative in [
            "guide.md/",
            "guide.md/.",
            "guide.md/child/..",
            "guide.md/%2E",
            "guide.md%2F",
        ] {
            assert!(resolve(&source, relative, &scope).is_none(), "{relative}");
            let absolute = format!("{}/{relative}", directory.uri(""));
            assert!(resolve(&source, &absolute, &scope).is_none(), "{absolute}");
        }
    }

    #[test]
    fn completes_one_directory_and_merges_unsaved_descendants_in_sorted_order() {
        let directory = TestDir::new();
        fs::create_dir(directory.0.join("docs")).unwrap();
        fs::write(directory.0.join("guide.md"), "# Guide").unwrap();
        fs::write(directory.0.join("guide 中文.md"), "# Guide").unwrap();
        fs::write(directory.0.join("docs/disk.md"), "# Disk").unwrap();
        let source = directory.uri("source.md");
        let scope = Workspace::default().scope(&source);
        let open = [
            directory.0.join("guide.md"),
            directory.0.join("docs/new.md"),
            directory.0.join("new/sub/file.md"),
        ];
        let names = |prefix| {
            path_candidates(&source, prefix, &scope, open.iter().map(PathBuf::as_path))
                .into_iter()
                .map(|entry| (entry.name, entry.directory))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names("gu"),
            vec![
                ("guide 中文.md".to_string(), false),
                ("guide.md".to_string(), false)
            ]
        );
        assert_eq!(names("guide%2"), vec![("guide 中文.md".to_string(), false)]);
        assert_eq!(
            names("docs/"),
            vec![
                ("disk.md".to_string(), false),
                ("new.md".to_string(), false)
            ]
        );
        assert_eq!(names("docs%2Fdi"), vec![("disk.md".to_string(), false)]);
        assert_eq!(names("docs%2fne"), vec![("new.md".to_string(), false)]);
        assert_eq!(names("ne"), vec![("new".to_string(), true)]);
        assert_eq!(names("new/"), vec![("sub".to_string(), true)]);
        fs::remove_file(directory.0.join("docs/disk.md")).unwrap();
        assert_eq!(names("docs/"), vec![("new.md".to_string(), false)]);
        assert!(names("../").is_empty());
        assert!(names("https://example.test/").is_empty());
        let opaque = directory.0.join("command:runner.md");
        assert!(path_candidates(
            &source,
            "command:run",
            &scope,
            std::iter::once(opaque.as_path())
        )
        .is_empty());
        assert!(path_candidates(
            "untitled:source",
            "",
            &scope,
            open.iter().map(PathBuf::as_path)
        )
        .is_empty());
    }

    #[test]
    fn completion_prefixes_accept_mixed_unicode_and_partial_percent_spellings() {
        for (value, prefix) in [
            ("中文.md", "%e4"),
            ("中文.md", "%E4%B8"),
            ("中文 file.md", "中文%2"),
            ("a b.md", "a%2"),
            ("guide x.md", "%67uide%2"),
            ("a+b.md", "a+"),
            ("a%b.md", "a%25"),
        ] {
            assert!(
                encode_component(value).starts_with(&component_prefix(prefix).unwrap()),
                "{value}: {prefix}"
            );
        }
        for prefix in ["%G", "%GG", "%2Z"] {
            assert!(component_prefix(prefix).is_none());
        }
        assert!(!encode_component("Guide.md").starts_with(&component_prefix("gu").unwrap()));
    }

    #[test]
    fn directory_completion_bounds_returned_names() {
        let directory = TestDir::new();
        let source = directory.uri("source.md");
        let scope = Workspace::default().scope(&source);
        let open: Vec<_> = (0..400)
            .rev()
            .map(|index| directory.0.join(format!("{index:04}.md")))
            .collect();
        let candidates = path_candidates(&source, "", &scope, open.iter().map(PathBuf::as_path));
        assert_eq!(candidates.len(), 200);
        assert_eq!(candidates[0].name, "0000.md");
        assert_eq!(candidates[199].name, "0199.md");
        let candidates = path_candidates(&source, "03", &scope, open.iter().map(PathBuf::as_path));
        assert_eq!(candidates.len(), 100);
        assert_eq!(candidates[0].name, "0300.md");
    }

    #[cfg(unix)]
    #[test]
    fn directory_completion_omits_names_and_symlink_targets_that_navigation_cannot_represent() {
        let directory = TestDir::new();
        for name in ["ordinary\nname.md", "ordinary\\name.md", "ordinary.md"] {
            fs::write(directory.0.join(name), "# Intro").unwrap();
        }
        std::os::unix::fs::symlink(
            directory.0.join("ordinary\nname.md"),
            directory.0.join("alias.md"),
        )
        .unwrap();
        let source = directory.uri("source.md");
        let scope = Workspace::default().scope(&source);
        let candidates = path_candidates(&source, "", &scope, std::iter::empty());
        assert_eq!(
            candidates
                .into_iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>(),
            ["ordinary.md"]
        );
        assert!(resolve(&source, "alias.md", &scope).is_none());
    }

    #[test]
    fn rejects_sensitive_path_spellings_before_io() {
        let root = std::env::temp_dir().join("yozora-lsp-no-io");
        let scope = FileScope {
            roots: vec![(root.clone(), root.clone())],
        };
        for relative in [
            ".ssh/guide.md",
            ".env",
            ".env.local.md",
            "local/env.production.md",
            ".git-credentials",
            "capture.http_request",
            "capture.http_response",
            ".aws/guide.md",
        ] {
            let path = root.join(relative);
            assert!(blocked_path(&path));
            assert!(scope.resolve(&path).is_none(), "{relative}");
            assert!(
                file_uri_path(&path_uri(&path).unwrap()).is_none(),
                "{relative}"
            );
        }
    }

    #[test]
    fn reads_only_bounded_regular_utf8_markdown_files_and_does_not_cache_them() {
        let directory = TestDir::new();
        let path = directory.0.join("guide.MD");
        fs::write(&path, "# Before").unwrap();
        assert_eq!(read_markdown(&path).as_deref(), Some("# Before"));
        fs::write(&path, "# After").unwrap();
        assert_eq!(read_markdown(&path).as_deref(), Some("# After"));
        fs::write(&path, [0xFF]).unwrap();
        assert!(read_markdown(&path).is_none());
        File::create(&path)
            .unwrap()
            .set_len((MAX_DOCUMENT_BYTES + 1) as u64)
            .unwrap();
        assert!(read_markdown(&path).is_none());
        assert!(read_markdown(&directory.0.join("missing.md")).is_none());
        fs::create_dir(directory.0.join("directory.md")).unwrap();
        assert!(read_markdown(&directory.0.join("directory.md")).is_none());
        fs::write(directory.0.join("plain.txt"), "# Text").unwrap();
        assert!(read_markdown(&directory.0.join("plain.txt")).is_none());
        #[cfg(unix)]
        {
            let path = directory.0.join("socket.md");
            let _socket = std::os::unix::net::UnixListener::bind(&path).unwrap();
            assert!(read_markdown(&path).is_none());
        }
    }

    #[test]
    fn workspace_traversal_is_sorted_deduplicated_and_skips_generated_trees() {
        let directory = TestDir::new();
        for path in ["docs", "target", "node_modules", ".cache", ".venv", ".git"] {
            fs::create_dir(directory.0.join(path)).unwrap();
            fs::write(directory.0.join(path).join("guide.md"), "# Heading").unwrap();
        }
        fs::write(directory.0.join("top.MARKDOWN"), "# Top").unwrap();
        fs::write(directory.0.join("ignored.txt"), "# Other").unwrap();
        let workspace = Workspace::new(vec![
            directory.uri(""),
            directory.uri("docs"),
            directory.uri(""),
        ]);
        let paths = workspace
            .index_scope(&Cancellation::default(), 100)
            .unwrap()
            .markdown_files(&Cancellation::default(), 100, 10)
            .unwrap();
        assert_eq!(
            paths,
            [
                directory.0.join("docs/guide.md"),
                directory.0.join("top.MARKDOWN")
            ]
        );
        for workspace in [
            Workspace::default(),
            Workspace::new(vec!["vscode-remote://host/docs".to_string()]),
        ] {
            assert!(workspace
                .index_scope(&Cancellation::default(), 100)
                .unwrap()
                .markdown_files(&Cancellation::default(), 0, 0)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn traversal_counts_skipped_entries_and_rejects_partial_results() {
        let directory = TestDir::new();
        fs::write(directory.0.join("a.txt"), "ignored").unwrap();
        fs::write(directory.0.join("b.txt"), "ignored").unwrap();
        let scope = Workspace::new(vec![directory.uri("")])
            .index_scope(&Cancellation::default(), 100)
            .unwrap();
        assert_eq!(
            scope
                .markdown_files(&Cancellation::default(), 1, 10)
                .unwrap_err()
                .code,
            -32000
        );
        fs::write(directory.0.join("first.md"), "# First").unwrap();
        fs::write(directory.0.join("second.md"), "# Second").unwrap();
        assert_eq!(
            scope
                .markdown_files(&Cancellation::default(), 10, 1)
                .unwrap_err()
                .code,
            -32000
        );
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert_eq!(
            scope.markdown_files(&cancellation, 0, 0).unwrap_err().code,
            -32800
        );
    }

    #[test]
    fn generated_tree_exclusions_are_relative_to_each_explicit_root() {
        let directory = TestDir::new();
        fs::create_dir_all(directory.0.join("target/project/node_modules")).unwrap();
        fs::write(directory.0.join("target/project/guide.md"), "# Guide").unwrap();
        fs::write(
            directory.0.join("target/project/node_modules/skipped.md"),
            "# Skipped",
        )
        .unwrap();
        for roots in [
            vec![directory.uri("target/project")],
            vec![directory.uri(""), directory.uri("target/project")],
        ] {
            let paths = Workspace::new(roots)
                .index_scope(&Cancellation::default(), 100)
                .unwrap()
                .markdown_files(&Cancellation::default(), 100, 10)
                .unwrap();
            assert_eq!(paths, [directory.0.join("target/project/guide.md")]);
        }
    }

    #[cfg(unix)]
    #[test]
    fn traversal_deduplicates_symlink_cycles_and_rechecks_escape_targets() {
        use std::os::unix::fs::symlink;
        let directory = TestDir::new();
        fs::create_dir_all(directory.0.join("root/docs")).unwrap();
        fs::create_dir(directory.0.join("outside")).unwrap();
        fs::write(directory.0.join("root/docs/guide.md"), "# Guide").unwrap();
        fs::write(directory.0.join("outside/outside.md"), "# Outside").unwrap();
        symlink("docs", directory.0.join("root/alias")).unwrap();
        symlink("..", directory.0.join("root/docs/cycle")).unwrap();
        symlink("../outside", directory.0.join("root/escape")).unwrap();
        symlink("missing", directory.0.join("root/dangling")).unwrap();
        symlink("docs/guide.md", directory.0.join("root/alias.md")).unwrap();
        let workspace = Workspace::new(vec![directory.uri("root"), directory.uri("root/alias")]);
        let paths = workspace
            .index_scope(&Cancellation::default(), 100)
            .unwrap()
            .markdown_files(&Cancellation::default(), 20, 10)
            .unwrap();
        assert_eq!(paths, [directory.0.join("root/docs/guide.md")]);
        fs::remove_file(directory.0.join("root/alias.md")).unwrap();
        symlink("../outside/outside.md", directory.0.join("root/alias.md")).unwrap();
        let paths = workspace
            .index_scope(&Cancellation::default(), 100)
            .unwrap()
            .markdown_files(&Cancellation::default(), 20, 10)
            .unwrap();
        assert_eq!(paths, [directory.0.join("root/docs/guide.md")]);
    }

    #[cfg(unix)]
    #[test]
    fn resolves_symlinks_inside_scope_and_rejects_escapes_and_dangling_links() {
        use std::os::unix::fs::symlink;
        let directory = TestDir::new();
        fs::create_dir_all(directory.0.join("root/docs")).unwrap();
        fs::create_dir(directory.0.join("outside")).unwrap();
        fs::write(directory.0.join("root/docs/guide.md"), "# Guide").unwrap();
        fs::write(directory.0.join("outside/guide.md"), "# Outside").unwrap();
        symlink(
            directory.0.join("root/docs"),
            directory.0.join("root/alias"),
        )
        .unwrap();
        symlink(directory.0.join("outside"), directory.0.join("root/escape")).unwrap();
        symlink(
            directory.0.join("absent"),
            directory.0.join("root/dangling"),
        )
        .unwrap();
        let source = directory.uri("root/source.md");
        let scope = Workspace::default().scope(&source);
        assert!(
            matches!(resolve(&source, "alias/guide.md#guide", &scope), Some(Target::Local { file: Some(file), .. })
            if file.path == directory.0.join("root/docs/guide.md")
                && file.uri == directory.uri("root/alias/guide.md"))
        );
        assert!(resolve(&source, "escape/guide.md", &scope).is_none());
        assert!(resolve(&source, "escape/unsaved.md", &scope).is_none());
        assert!(resolve(&source, "dangling/new.md", &scope).is_none());
        assert_eq!(
            path_candidates(&source, "", &scope, std::iter::empty())
                .into_iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>(),
            ["alias", "docs"]
        );
        assert!(path_candidates(&source, "escape/", &scope, std::iter::empty()).is_empty());
        fs::remove_file(directory.0.join("root/alias")).unwrap();
        symlink(directory.0.join("outside"), directory.0.join("root/alias")).unwrap();
        assert!(resolve(&source, "alias/guide.md", &scope).is_none());
    }
}
