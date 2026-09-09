use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use yozora_ast::Node;
use yozora_ast_util::calc_heading_identifiers;
use yozora_parser::YozoraParser;

use crate::analysis::{heading_selection_range, nodes, plain_text};
use crate::cancellation::Cancellation;
use crate::document::{Document, Snapshot};
use crate::files::{self, FileScope, SourceBuffer, Workspace};
use crate::protocol::{FileRename, Position, PrepareRenameResult, Range, ResponseError, TextEdit};
use crate::resource_edit::{self, failed, Budget, HeadingChange, Replacement};

pub struct DocumentEdits {
    pub uri: String,
    pub version: Option<i32>,
    pub edits: Vec<TextEdit>,
}

/// The worker owns these document snapshots. Refactors return edits and keep
/// source text unchanged; the client owns edit application and filesystem moves.
pub struct Context<'a> {
    pub workspace: &'a Workspace,
    pub documents: &'a mut HashMap<String, Document>,
    pub parser: &'a YozoraParser,
    pub cancellation: &'a Cancellation,
    pub used_buffers: &'a mut HashSet<String>,
    pub heading_id_prefix: &'a str,
    pub index: &'a mut Index,
    pub catalog: &'a mut files::Catalog,
    pub revision: Option<u64>,
    pub validate_disk: bool,
}

struct CachedSource {
    uri: String,
    text: Arc<String>,
    summary: resource_edit::Summary,
}

impl CachedSource {
    fn owned_bytes(&self, path: &PathBuf) -> usize {
        std::mem::size_of::<Self>()
            + path.capacity()
            + self.uri.capacity()
            + self.text.capacity()
            + self.summary.owned_bytes()
    }
}

/// Worker-owned source summaries, independent of the selected heading, file
/// move, and replacement name. Final disk reads are the default; confirmed file
/// events can validate snapshots when the client explicitly opts in.
#[derive(Default)]
pub struct Index {
    entries: HashMap<PathBuf, CachedSource>,
    bytes: usize,
    revision: Option<u64>,
    catalog: u64,
}

impl Index {
    fn take(&mut self, path: &Path) -> Option<CachedSource> {
        self.entries.remove_entry(path).map(|(path, source)| {
            self.bytes -= source.owned_bytes(&path);
            source
        })
    }

    fn insert(&mut self, path: PathBuf, source: CachedSource) {
        let bytes = source.owned_bytes(&path);
        let capacity = 40 * 1024 * 1024;
        if bytes > capacity {
            return;
        }
        if self.bytes + bytes > capacity {
            self.entries.clear();
            self.bytes = 0;
        }
        self.bytes += bytes;
        self.entries.insert(path, source);
    }
}

struct HeadingSubject {
    node_start: Position,
    range: Range,
    placeholder: String,
    empty: bool,
}

fn subject(snapshot: &Snapshot<'_>, position: Position) -> Option<HeadingSubject> {
    nodes(&snapshot.root.children)
        .filter_map(|node| {
            let Node::Heading(heading) = node else {
                return None;
            };
            let node_range = Range::from(heading.position.as_ref()?);
            let empty = heading.children.is_empty();
            let range = if empty {
                let start = snapshot
                    .lines
                    .byte_offset(snapshot.text, node_range.start)
                    .ok()?;
                let end = snapshot
                    .lines
                    .byte_offset(snapshot.text, node_range.end)
                    .ok()?;
                let text = snapshot.text.get(start..end)?;
                let marker = text.find('#')?;
                let point = snapshot
                    .lines
                    .position(snapshot.text, start + marker + heading.depth as usize)
                    .ok()?;
                Range {
                    start: point,
                    end: point,
                }
            } else {
                heading_selection_range(heading, node_range)
            };
            (range.start <= position && position <= range.end).then(|| HeadingSubject {
                node_start: node_range.start,
                range,
                placeholder: plain_text(&heading.children),
                empty,
            })
        })
        .last()
}

pub fn prepare(snapshot: &Snapshot<'_>, position: Position) -> Option<PrepareRenameResult> {
    let subject = subject(snapshot, position)?;
    Some(PrepareRenameResult {
        range: subject.range,
        placeholder: subject.placeholder,
    })
}

struct HeadingPlan {
    uri: String,
    path: Option<PathBuf>,
    change: HeadingChange,
    identifiers: BTreeMap<String, String>,
    activated: BTreeSet<String>,
}

struct FileMove {
    old: PathBuf,
    new: PathBuf,
    new_uri: String,
}

enum Plan {
    Heading(HeadingPlan),
    Files(Vec<FileMove>),
}

impl Plan {
    fn heading(&self, uri: &str) -> Option<&HeadingChange> {
        match self {
            Self::Heading(plan) if plan.uri == uri => Some(&plan.change),
            _ => None,
        }
    }

    fn destination(
        &self,
        source: &SourceBuffer,
        url: &str,
        resolver: &mut files::Resolver<'_>,
    ) -> Result<Option<String>, ResponseError> {
        if let Self::Heading(plan) = self {
            let Some((_, fragment)) = url.split_once('#') else {
                return Ok(None);
            };
            // Unescaped fragments already have their final spelling. Resolving
            // an unrelated destination cannot contribute to this heading plan.
            if !fragment.contains('%')
                && !plan.identifiers.contains_key(fragment)
                && !plan.activated.contains(fragment)
            {
                return Ok(None);
            }
        }
        let Some(files::Target::Local { file, fragment, .. }) = resolver.resolve(&source.uri, url)
        else {
            return Ok(None);
        };
        match self {
            Self::Heading(plan) => {
                let points_here = match file {
                    Some(file) => plan.path.as_ref() == Some(&file.path),
                    None => {
                        plan.uri == source.uri || (plan.path.is_some() && plan.path == source.path)
                    }
                };
                let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) else {
                    return Ok(None);
                };
                if !points_here {
                    return Ok(None);
                }
                if plan.activated.contains(&fragment) {
                    return Err(failed(
                        "rename would activate a previously missing heading anchor",
                    ));
                }
                Ok(plan.identifiers.get(&fragment).map(|identifier| {
                    let resource = url
                        .split_once('#')
                        .expect("a resolved fragment has a delimiter")
                        .0;
                    format!("{resource}#{}", files::encode_component(identifier))
                }))
            }
            Self::Files(moves) => {
                let Some(file) = file else { return Ok(None) };
                let target = moves
                    .iter()
                    .find(|item| item.old == file.path)
                    .map_or(&file.path, |item| &item.new);
                let new_source = moves
                    .iter()
                    .find(|item| source.path.as_ref() == Some(&item.old));
                let new_source_uri =
                    new_source.map_or(source.uri.as_str(), |item| item.new_uri.as_str());
                if matches!(resolver.resolve(new_source_uri, url), Some(files::Target::Local { file: Some(file), .. }) if &file.path == target)
                {
                    return Ok(None);
                }
                let end = url.find(['?', '#']).unwrap_or(url.len());
                let resource = &url[..end];
                let path = if resource
                    .split_once(':')
                    .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
                {
                    files::path_uri(target)
                        .ok_or_else(|| failed("renamed path cannot form a file URI"))?
                } else if resource.starts_with('/') {
                    files::path_uri(target)
                        .and_then(|uri| uri.strip_prefix("file://").map(str::to_string))
                        .ok_or_else(|| failed("renamed path cannot form an absolute URI path"))?
                } else {
                    let source = files::file_uri_path(new_source_uri)
                        .ok_or_else(|| failed("relative file links require a local source URI"))?;
                    relative_uri(
                        source
                            .parent()
                            .ok_or_else(|| failed("source has no parent directory"))?,
                        target,
                    )?
                };
                Ok(Some(format!("{path}{}", &url[end..])))
            }
        }
    }
}

impl Context<'_> {
    pub fn heading(
        &mut self,
        uri: &str,
        position: Position,
        name: &str,
    ) -> Result<Vec<DocumentEdits>, ResponseError> {
        self.cancellation.check()?;
        if name.is_empty()
            || name.trim() != name
            || name.chars().count() > 1_000
            || name.chars().any(char::is_control)
        {
            return Err(ResponseError::invalid_params("heading names must be nonempty single-line text of at most 1000 characters, without surrounding whitespace"));
        }
        self.used_buffers.insert(uri.to_string());
        let document = self
            .documents
            .get_mut(uri)
            .ok_or_else(|| failed("heading document is not open"))?;
        let snapshot = document.snapshot(self.parser, position)?;
        self.cancellation.check()?;
        let subject =
            subject(&snapshot, position).ok_or_else(|| failed("position is not heading text"))?;
        if snapshot
            .root
            .children
            .iter()
            .filter(|node| matches!(node, Node::Heading(_)))
            .take(20_001)
            .count()
            > 20_000
        {
            return Err(ResponseError::new(
                -32000,
                "heading rename exceeds the heading count limit",
            ));
        }
        if name == subject.placeholder {
            return Ok(Vec::new());
        }
        let mut text = if subject.empty {
            " ".to_string()
        } else {
            String::new()
        };
        for character in name.chars() {
            if character.is_ascii_punctuation() {
                text.push('\\');
            }
            text.push(character);
        }
        let change = HeadingChange {
            node_start: subject.node_start,
            range: subject.range,
            replacement: Replacement {
                range: snapshot
                    .lines
                    .byte_offset(snapshot.text, subject.range.start)?
                    ..snapshot
                        .lines
                        .byte_offset(snapshot.text, subject.range.end)?,
                text,
            },
            name: name.to_string(),
        };
        let mut budget = Budget::default();
        let root = resource_edit::heading_preview(
            &snapshot,
            &change,
            self.parser,
            self.cancellation,
            &mut budget,
        )?;
        let before = calc_heading_identifiers(snapshot.root, self.heading_id_prefix);
        let after = calc_heading_identifiers(&root, self.heading_id_prefix);
        let changed = before != after;
        let known: HashSet<_> = before.iter().map(String::as_str).collect();
        let activated = after
            .iter()
            .filter(|identifier| !known.contains(identifier.as_str()))
            .cloned()
            .collect();
        let mut identifiers = BTreeMap::new();
        for (old, new) in before.into_iter().zip(after) {
            if old != new && !old.is_empty() {
                if new.is_empty() {
                    return Err(failed("rename would remove an addressable heading anchor"));
                }
                identifiers.insert(old, new);
            }
        }
        let path = files::file_uri_path(uri);
        let mut plan = Plan::Heading(HeadingPlan {
            uri: uri.to_string(),
            path: None,
            change,
            identifiers,
            activated,
        });
        if !changed || path.is_none() {
            let scope =
                Workspace::default().index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
            let source = SourceBuffer {
                uri: uri.to_string(),
                path: None,
                ambiguous: false,
            };
            let mut resolver = files::Resolver::new(&scope);
            let edits = resource_edit::rewrite(
                &snapshot,
                plan.heading(uri),
                |url| plan.destination(&source, url, &mut resolver),
                self.parser,
                self.cancellation,
                &mut budget,
            )?;
            budget.document(uri)?;
            return Ok(vec![DocumentEdits {
                uri: uri.to_string(),
                version: Some(snapshot.version),
                edits,
            }]);
        }
        let scope = self
            .workspace
            .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
        let path = scope.resolve(&path.unwrap()).ok_or_else(|| {
            failed(
                "heading anchor changes require a local workspace root containing the source file",
            )
        })?;
        if let Plan::Heading(plan) = &mut plan {
            plan.path = Some(path);
        }
        // Release the preview AST before scanning other documents.
        drop(root);
        self.execute(scope, plan, budget)
    }

    pub fn files(&mut self, files: &[FileRename]) -> Result<Vec<DocumentEdits>, ResponseError> {
        self.cancellation.check()?;
        let scope = self
            .workspace
            .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
        let mut moves = Vec::new();
        let mut old_paths = BTreeSet::new();
        let mut new_paths = BTreeSet::new();
        for file in files {
            self.cancellation.check()?;
            let old = operation_path(&scope, &file.old_uri)?;
            let new = operation_path(&scope, &file.new_uri)?;
            if old == new {
                continue;
            }
            if !old_paths.insert(old.clone()) || !new_paths.insert(new.clone()) {
                return Err(failed(
                    "file rename sources and destinations must be unique",
                ));
            }
            if !files::is_file(&old) {
                return Err(failed("file rename requires an existing regular file"));
            }
            moves.push(FileMove {
                old,
                new,
                new_uri: file.new_uri.clone(),
            });
        }
        for item in &moves {
            if item
                .new
                .try_exists()
                .map_err(|_| failed("cannot inspect a file rename destination"))?
                && !old_paths.contains(&item.new)
            {
                return Err(failed("file rename destination already exists"));
            }
        }
        if moves.is_empty() {
            return Ok(Vec::new());
        }
        self.execute(scope, Plan::Files(moves), Budget::default())
    }

    fn execute(
        &mut self,
        scope: FileScope,
        plan: Plan,
        mut budget: Budget,
    ) -> Result<Vec<DocumentEdits>, ResponseError> {
        let validate_disk = self.validate_disk || self.revision.is_none();
        let (sources, reused) = self.catalog.sources(
            &scope,
            self.documents.keys().map(String::as_str),
            self.cancellation,
            files::MAX_WORKSPACE_ENTRIES,
            files::MAX_WORKSPACE_FILES,
            self.revision,
        )?;
        let generation = self.catalog.generation();
        let previous_revision = self.index.revision;
        let reuse_contents = reused
            && self.revision.is_some()
            && previous_revision.is_some()
            && self.index.catalog == generation;
        self.index.revision = None;
        if sources.buffers.iter().any(|buffer| buffer.ambiguous) {
            return Err(failed(
                "close duplicate URI aliases before a workspace rename",
            ));
        }
        let initial_files: BTreeSet<_> = sources.files.iter().cloned().collect();
        if !reuse_contents {
            self.index
                .entries
                .retain(|path, _| initial_files.contains(path));
            self.index.bytes = self
                .index
                .entries
                .iter()
                .map(|(path, source)| source.owned_bytes(path))
                .sum();
        }
        let mut remaining = files::MAX_WORKSPACE_BYTES;
        let mut result = Vec::new();
        let mut snapshots = Vec::new();
        let mut operation = Operation {
            plan: &plan,
            resolver: files::Resolver::new(&scope),
            parser: self.parser,
            cancellation: self.cancellation,
            budget: &mut budget,
        };
        for source in &sources.buffers {
            self.cancellation.check()?;
            self.used_buffers.insert(source.uri.clone());
            if let Plan::Files(moves) = &plan {
                if moves.iter().any(|item| {
                    source.path.as_ref() == Some(&item.old)
                        && files::file_uri_path(&source.uri).as_ref() != Some(&item.old)
                }) {
                    return Err(failed(
                        "close symlink alias buffers before moving their target file",
                    ));
                }
            }
            let document = self
                .documents
                .get_mut(&source.uri)
                .expect("selected buffer is open");
            remaining = remaining
                .checked_sub(document.text()?.len())
                .ok_or_else(|| failed("workspace rename exceeds the source byte limit"))?;
            let version = Some(document.version());
            let edits = operation.edit(document, source)?;
            if !edits.is_empty() {
                operation.budget.document(&source.uri)?;
                result.push(DocumentEdits {
                    uri: source.uri.clone(),
                    version,
                    edits,
                });
            }
        }
        for path in sources.files {
            self.cancellation.check()?;
            let cached = self.index.take(&path);
            let text = if let Some(cached) = cached.as_ref().filter(|_| {
                reuse_contents
                    && self
                        .catalog
                        .contents_unchanged(&path, previous_revision, self.revision)
            }) {
                let text = Arc::clone(&cached.text);
                remaining = remaining
                    .checked_sub(text.len())
                    .ok_or_else(|| failed("workspace rename exceeds the source byte limit"))?;
                text
            } else {
                if scope.resolve(&path).as_ref() != Some(&path) {
                    return Err(modified());
                }
                Arc::new(
                    files::read_markdown_budgeted(&path, &mut remaining)?.ok_or_else(|| {
                        failed("cannot read every Markdown file required for this rename")
                    })?,
                )
            };
            let cached = cached.filter(|source| source.text.as_str() == text.as_str());
            let uri = cached.as_ref().map_or_else(
                || files::path_uri(&path).expect("scoped paths form file URIs"),
                |source| source.uri.clone(),
            );
            let source = SourceBuffer {
                uri: uri.clone(),
                path: Some(path.clone()),
                ambiguous: false,
            };
            operation.budget.parse(text.len())?;
            let mut summary = cached.map(|source| source.summary);
            let mut document = None;
            if summary.is_none() {
                let mut parsed = Document::new(0, text.as_ref().clone())?;
                summary = resource_edit::Summary::new(
                    &parsed.snapshot(self.parser, Position::default())?,
                    self.cancellation,
                )?;
                document = Some(parsed);
            }
            let edits = match summary
                .as_ref()
                .map(|summary| {
                    summary.rewrite(
                        &text,
                        None,
                        |url| {
                            operation
                                .plan
                                .destination(&source, url, &mut operation.resolver)
                        },
                        self.cancellation,
                        operation.budget,
                    )
                })
                .transpose()?
                .flatten()
            {
                Some(edits) => edits,
                None => {
                    let document = match &mut document {
                        Some(document) => document,
                        None => document.insert(Document::new(0, text.as_ref().clone())?),
                    };
                    operation.rewrite(document, &source)?
                }
            };
            if !edits.is_empty() {
                operation.budget.document(&uri)?;
                result.push(DocumentEdits {
                    uri: uri.clone(),
                    version: None,
                    edits,
                });
            }
            if validate_disk {
                snapshots.push((path.clone(), Arc::clone(&text)));
            }
            if let Some(summary) = summary {
                self.index.insert(path, CachedSource { uri, text, summary });
            }
        }
        // Inventory follows the acknowledged watch contract. By default, closed
        // text is additionally reread; only an explicit option can skip that read.
        let current_scope = self
            .workspace
            .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
        if current_scope != scope {
            return Err(modified());
        }
        // With an acknowledged watch, the server rejects results if an event
        // changed the workspace epoch. Strict clients also rediscover inventory.
        if self.revision.is_none() {
            let current = scope.sources(
                self.documents.keys().map(String::as_str),
                self.cancellation,
                files::MAX_WORKSPACE_ENTRIES,
                files::MAX_WORKSPACE_FILES,
            )?;
            if current.files.into_iter().collect::<BTreeSet<_>>() != initial_files
                || current.buffers != sources.buffers
            {
                return Err(modified());
            }
        }
        validate_sources(&snapshots, &scope, self.cancellation)?;
        if let Plan::Files(moves) = &plan {
            for item in moves {
                if !files::is_file(&item.old)
                    || scope.resolve(&item.old).as_ref() != Some(&item.old)
                    || scope.resolve(&item.new).as_ref() != Some(&item.new)
                    || (item.new.try_exists().map_err(|_| modified())?
                        && !moves.iter().any(|other| other.old == item.new))
                {
                    return Err(modified());
                }
            }
        }
        if !operation.resolver.unchanged(self.cancellation)? {
            return Err(modified());
        }
        self.cancellation.check()?;
        result.sort_by(|left, right| left.uri.cmp(&right.uri));
        self.index.revision = self.revision;
        self.index.catalog = generation;
        Ok(result)
    }
}

fn validate_sources(
    snapshots: &[(PathBuf, Arc<String>)],
    scope: &FileScope,
    cancellation: &Cancellation,
) -> Result<(), ResponseError> {
    cancellation.check()?;
    let validate = |sources: &[(PathBuf, Arc<String>)]| {
        // Each partition spends only its share of the original source bytes.
        // Growth beyond that share necessarily means the snapshot changed.
        let mut remaining = sources.iter().map(|(_, text)| text.len()).sum();
        for (path, expected) in sources {
            cancellation.check()?;
            let actual = files::read_scoped_markdown_budgeted(path, scope, &mut remaining)
                .map_err(|_| modified())?;
            if actual.as_deref() != Some(expected.as_str()) {
                return Err(modified());
            }
        }
        Ok(())
    };
    let workers = if snapshots.len() >= 128 {
        std::thread::available_parallelism().map_or(1, |count| count.get().min(4))
    } else {
        1
    };
    if workers == 1 {
        return validate(snapshots);
    }
    // Parsing remains on the owning worker. These bounded, short-lived readers
    // start after every edit is planned and all finish before publication.
    std::thread::scope(|threads| {
        let mut chunks = snapshots.chunks(snapshots.len().div_ceil(workers));
        let first = chunks.next().unwrap();
        let mut pending = Vec::new();
        let mut result = Ok(());
        for chunk in chunks {
            match std::thread::Builder::new()
                .name("yozora-lsp-file-check".into())
                .spawn_scoped(threads, move || validate(chunk))
            {
                Ok(reader) => pending.push(reader),
                Err(_) => result = result.and(validate(chunk)),
            }
        }
        result = result.and(validate(first));
        for reader in pending {
            let checked = reader
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            result = result.and(checked);
        }
        result
    })
}

struct Operation<'a> {
    plan: &'a Plan,
    resolver: files::Resolver<'a>,
    parser: &'a YozoraParser,
    cancellation: &'a Cancellation,
    budget: &'a mut Budget,
}

impl Operation<'_> {
    fn edit(
        &mut self,
        document: &mut Document,
        source: &SourceBuffer,
    ) -> Result<Vec<TextEdit>, ResponseError> {
        self.cancellation.check()?;
        self.budget.parse(document.text()?.len())?;
        let snapshot = document.snapshot(self.parser, Position::default())?;
        if let Some(summary) = resource_edit::Summary::new(&snapshot, self.cancellation)? {
            if let Some(edits) = summary.rewrite(
                snapshot.text,
                self.plan.heading(&source.uri),
                |url| self.plan.destination(source, url, &mut self.resolver),
                self.cancellation,
                self.budget,
            )? {
                return Ok(edits);
            }
        }
        self.rewrite(document, source)
    }

    fn rewrite(
        &mut self,
        document: &mut Document,
        source: &SourceBuffer,
    ) -> Result<Vec<TextEdit>, ResponseError> {
        let snapshot = document.snapshot(self.parser, Position::default())?;
        self.cancellation.check()?;
        resource_edit::rewrite(
            &snapshot,
            self.plan.heading(&source.uri),
            |url| self.plan.destination(source, url, &mut self.resolver),
            self.parser,
            self.cancellation,
            self.budget,
        )
    }
}

fn operation_path(scope: &FileScope, uri: &str) -> Result<PathBuf, ResponseError> {
    if uri.contains(['?', '#']) {
        return Err(ResponseError::invalid_params(
            "file rename URIs cannot contain queries or fragments",
        ));
    }
    let Some(files::Target::Local {
        file: Some(file), ..
    }) = files::resolve(uri, uri, scope)
    else {
        return Err(failed(
            "file rename paths must be local files inside workspace roots",
        ));
    };
    if files::file_uri_path(uri).as_ref() != Some(&file.path) {
        return Err(failed(
            "file operations require canonical paths; symlink aliases are not renamed",
        ));
    }
    Ok(file.path)
}

fn relative_uri(parent: &Path, target: &Path) -> Result<String, ResponseError> {
    let from: Vec<_> = parent.components().collect();
    let to: Vec<_> = target.components().collect();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return files::path_uri(target).ok_or_else(|| failed("renamed path cannot form a URI"));
    }
    let mut components = vec!["..".to_string(); from.len() - common];
    for component in &to[common..] {
        let Component::Normal(value) = component else {
            return Err(failed("cannot make a relative file URI"));
        };
        components.push(files::encode_component(
            value
                .to_str()
                .ok_or_else(|| failed("file path is not Unicode"))?,
        ));
    }
    let path = components.join("/");
    if path.is_empty() {
        return Ok("./".to_string());
    }
    Ok(
        if path
            .split('/')
            .next()
            .is_some_and(|component| component.contains(':'))
        {
            format!("./{path}")
        } else {
            path
        },
    )
}

fn modified() -> ResponseError {
    ResponseError::new(
        -32801,
        "workspace files changed during rename; retry against the current files",
    )
}

#[cfg(test)]
mod tests;
