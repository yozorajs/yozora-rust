use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use yozora_ast::{Node, Root};
use yozora_parser::YozoraParser;

use crate::analysis::heading_selection_range;
use crate::cancellation::Cancellation;
use crate::document::{open_document, Document};
use crate::files::{self, FileScope, Target, Workspace};
use crate::links;
use crate::protocol::{Location, Position, Range, ResponseError};

use crate::reference_index::{
    Index, Key, Resolved as CachedResolution, ResolvedResource, MAX_RESOURCES, MAX_URL_BYTES,
};

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
}

enum Selection {
    Heading { identifier: String, range: Range },
    Destination(String),
}

struct Subject {
    key: Key,
    identifier: String,
    declaration: Location,
}

struct Resolved {
    subject: Subject,
    // Reuse a closed target's initial parse when scanning its own references.
    disk: Option<Document>,
}

struct Budget {
    text_bytes: usize,
    resources: usize,
    url_bytes: usize,
    locations: usize,
    location_bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            text_bytes: files::MAX_WORKSPACE_BYTES,
            resources: MAX_RESOURCES,
            url_bytes: MAX_URL_BYTES,
            locations: 10_000,
            location_bytes: 4 * 1024 * 1024,
        }
    }
}

impl Context<'_> {
    pub fn find(
        &mut self,
        source_uri: &str,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, ResponseError> {
        self.cancellation.check()?;
        let document = open_document(self.documents, source_uri)?;
        document.validate_position(position)?;
        let root = document.ast(self.parser)?;
        self.cancellation.check()?;
        let selection = if let Some(url) = links::destination_at(root, position) {
            if url.len() > MAX_URL_BYTES {
                return Err(limit("destination byte"));
            }
            Selection::Destination(url.to_string())
        } else {
            // Only top-level headings have TOC IDs. Avoid building every ID
            // when the cursor is outside a heading's text.
            if !root.children.iter().any(|node| {
                matches!(node, Node::Heading(heading) if heading.position.as_ref().is_some_and(|range| {
                    let range = heading_selection_range(heading, Range::from(range));
                    range.start <= position && position <= range.end
                }))
            }) {
                return Ok(Vec::new());
            }
            let Some((identifier, range)) =
                headings(root, self.heading_id_prefix, self.cancellation)?
                    .find(|(_, range)| range.start <= position && position <= range.end)
            else {
                return Ok(Vec::new());
            };
            if identifier.is_empty() {
                return Ok(Vec::new());
            }
            Selection::Heading { identifier, range }
        };
        let scope = self
            .workspace
            .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
        let open_uris =
            scope.open_file_uris(self.documents.keys().map(String::as_str), self.cancellation)?;
        let mut budget = Budget::default();
        let resolved = match selection {
            Selection::Heading { identifier, range } => Resolved {
                subject: Subject {
                    key: Key::Buffer(source_uri.to_string()),
                    identifier,
                    declaration: Location {
                        uri: source_uri.to_string(),
                        range,
                    },
                },
                disk: None,
            },
            Selection::Destination(url) => {
                budget.url_bytes -= url.len();
                let Some(resolved) =
                    self.resolve(source_uri, &url, &scope, &open_uris, &mut budget)?
                else {
                    return Ok(Vec::new());
                };
                resolved
            }
        };
        let Resolved { subject, mut disk } = resolved;
        let local = matches!(&subject.key, Key::Buffer(uri) if !open_uris.values().flatten().any(|open| open == uri));
        let mut buffers = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut reuse_contents = false;
        let mut previous_revision = None;
        if !local {
            let (sources, reused) = self.catalog.sources(
                &scope,
                self.documents.keys().map(String::as_str),
                self.cancellation,
                files::MAX_WORKSPACE_ENTRIES,
                files::MAX_WORKSPACE_FILES,
                self.revision,
            )?;
            previous_revision = self.index.revision(self.catalog.generation());
            reuse_contents = reused && previous_revision.is_some() && self.revision.is_some();
            buffers.extend(sources.buffers.into_iter().map(|buffer| buffer.uri));
            // Distinct open aliases may have different unsaved contents. Scan
            // each client URI, while disk files remain overlaid and deduplicated.
            buffers.extend(open_uris.values().flatten().cloned());
            buffers.insert(source_uri.to_string());
            paths.extend(sources.files);
        }
        // Untitled buffers and files outside the roots are only addressable
        // through same-buffer anchors; unrelated documents cannot refer to them.
        match &subject.key {
            Key::Buffer(uri) => {
                buffers.insert(uri.clone());
            }
            Key::File(path) => {
                paths.insert(path.clone());
            }
        }
        if buffers.len() + paths.len() > files::MAX_WORKSPACE_FILES {
            return Err(limit("file/buffer count"));
        }
        if !reuse_contents {
            self.index.retain(&paths);
        }
        self.index.finish_revision(None, self.catalog.generation());
        let mut search = Search {
            subject: &subject,
            resolver: files::Resolver::new(&scope),
            open_uris: &open_uris,
            cancellation: self.cancellation,
            budget,
            locations: Vec::new(),
        };
        for uri in buffers {
            self.cancellation.check()?;
            self.used_buffers.insert(uri.clone());
            let document = open_document(self.documents, &uri)?;
            search.budget.text_bytes = search
                .budget
                .text_bytes
                .checked_sub(document.text()?.len())
                .ok_or_else(|| limit("source byte"))?;
            let root = document.ast(self.parser)?;
            self.cancellation.check()?;
            search.collect(&uri, root, None)?;
        }
        for path in paths {
            self.cancellation.check()?;
            if reuse_contents
                && subject.key != Key::File(path.clone())
                && self
                    .catalog
                    .contents_unchanged(&path, previous_revision, self.revision)
            {
                if let Some((bytes, uri, resolved)) =
                    self.index
                        .resolved(&path, self.revision.unwrap(), self.catalog)
                {
                    search.budget.text_bytes = search
                        .budget
                        .text_bytes
                        .checked_sub(bytes)
                        .ok_or_else(|| limit("source byte"))?;
                    search.collect_resolved(uri, resolved)?;
                    continue;
                }
                if let Some((bytes, resources)) = self.index.cached(&path) {
                    search.budget.text_bytes = search
                        .budget
                        .text_bytes
                        .checked_sub(bytes)
                        .ok_or_else(|| limit("source byte"))?;
                    let uri = files::path_uri(&path).expect("scoped paths form local file URIs");
                    let resolved = search.collect_resources(
                        &uri,
                        Some(&path),
                        resources
                            .iter()
                            .map(|resource| (resource.range, resource.url.as_ref())),
                        true,
                    )?;
                    if let Some(resolved) = resolved {
                        self.index.cache_resolved(
                            &path,
                            self.revision.unwrap(),
                            self.catalog.generation(),
                            resolved,
                        );
                    }
                    continue;
                }
            }
            if scope.resolve(&path).as_ref() != Some(&path) {
                return Err(modified());
            }
            let uri = files::path_uri(&path).expect("scoped paths form local file URIs");
            if subject.key == Key::File(path.clone()) {
                let mut document = disk.take().expect("a closed target has a parsed snapshot");
                let root = document.ast(self.parser)?;
                self.cancellation.check()?;
                search.collect(&uri, root, Some(&path))?;
            } else {
                let Some(text) =
                    files::read_markdown_budgeted(&path, &mut search.budget.text_bytes)?
                else {
                    self.index.remove(&path);
                    continue;
                };
                let resources =
                    self.index
                        .resources(&path, text, self.parser, self.cancellation)?;
                let resolved = search.collect_resources(
                    &uri,
                    Some(&path),
                    resources
                        .iter()
                        .map(|resource| (resource.range, resource.url.as_ref())),
                    self.revision.is_some(),
                )?;
                if let (Some(revision), Some(resolved)) = (self.revision, resolved) {
                    self.index
                        .cache_resolved(&path, revision, self.catalog.generation(), resolved);
                }
            }
        }
        if include_declaration {
            search.push(&subject.declaration.uri, subject.declaration.range)?;
        }
        if !search.resolver.unchanged(self.cancellation)?
            || scope
                != self
                    .workspace
                    .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?
        {
            return Err(modified());
        }
        self.cancellation.check()?;
        search.locations.sort_by(|left, right| {
            left.uri
                .cmp(&right.uri)
                .then_with(|| left.range.start.cmp(&right.range.start))
                .then_with(|| left.range.end.cmp(&right.range.end))
        });
        search
            .locations
            .dedup_by(|left, right| left.uri == right.uri && left.range == right.range);
        self.index
            .finish_revision(self.revision, self.catalog.generation());
        Ok(search.locations)
    }

    fn resolve(
        &mut self,
        source_uri: &str,
        url: &str,
        scope: &FileScope,
        open_uris: &HashMap<PathBuf, Vec<String>>,
        budget: &mut Budget,
    ) -> Result<Option<Resolved>, ResponseError> {
        let Some(Target::Local {
            file,
            fragment: Some(fragment),
            ..
        }) = files::resolve(source_uri, url, scope)
        else {
            return Ok(None);
        };
        if fragment.is_empty() {
            return Ok(None);
        }
        let key = match file {
            Some(file) => match files::open_file_uri(open_uris, &file, source_uri) {
                Some(uri) => Key::Buffer(uri.to_string()),
                None => Key::File(file.path),
            },
            None => Key::Buffer(source_uri.to_string()),
        };
        let mut disk = None;
        let (uri, document) = match &key {
            Key::Buffer(uri) => {
                self.used_buffers.insert(uri.clone());
                (uri.clone(), open_document(self.documents, uri)?)
            }
            Key::File(path) => {
                if scope.resolve(path).as_ref() != Some(path) {
                    return Err(modified());
                }
                let Some(text) = files::read_markdown_budgeted(path, &mut budget.text_bytes)?
                else {
                    return Ok(None);
                };
                disk = Some(Document::new(0, text)?);
                (
                    files::path_uri(path).expect("scoped paths form local file URIs"),
                    disk.as_mut().unwrap(),
                )
            }
        };
        self.cancellation.check()?;
        let root = document.ast(self.parser)?;
        let Some((_, range)) = headings(root, self.heading_id_prefix, self.cancellation)?
            .find(|(identifier, _)| *identifier == fragment)
        else {
            return Ok(None);
        };
        Ok(Some(Resolved {
            subject: Subject {
                key,
                identifier: fragment,
                declaration: Location { uri, range },
            },
            disk,
        }))
    }
}

struct Search<'a> {
    subject: &'a Subject,
    resolver: files::Resolver<'a>,
    open_uris: &'a HashMap<PathBuf, Vec<String>>,
    cancellation: &'a Cancellation,
    budget: Budget,
    locations: Vec<Location>,
}

impl Search<'_> {
    fn collect(
        &mut self,
        uri: &str,
        root: &Root,
        path: Option<&std::path::Path>,
    ) -> Result<(), ResponseError> {
        self.collect_resources(uri, path, links::resources(root), false)
            .map(|_| ())
    }

    fn collect_resolved(
        &mut self,
        uri: &str,
        resolved: &CachedResolution,
    ) -> Result<(), ResponseError> {
        self.budget.resources = self
            .budget
            .resources
            .checked_sub(resolved.resources)
            .ok_or_else(|| limit("resource count"))?;
        self.budget.url_bytes = self
            .budget
            .url_bytes
            .checked_sub(resolved.url_bytes)
            .ok_or_else(|| limit("destination byte"))?;
        for entry in &resolved.entries {
            self.cancellation.check()?;
            if entry.fragment.as_ref() == self.subject.identifier
                && entry.target == self.subject.key
            {
                self.push(uri, entry.range)?;
            }
        }
        Ok(())
    }

    fn collect_resources<'a>(
        &mut self,
        uri: &str,
        source_path: Option<&std::path::Path>,
        resources: impl Iterator<Item = (Range, &'a str)>,
        cache: bool,
    ) -> Result<Option<CachedResolution>, ResponseError> {
        self.cancellation.check()?;
        let mut cached = cache.then(Vec::new);
        let mut cache_bytes = 0;
        let before_resources = self.budget.resources;
        let before_urls = self.budget.url_bytes;
        for (range, url) in resources {
            self.cancellation.check()?;
            self.budget.resources = self
                .budget
                .resources
                .checked_sub(1)
                .ok_or_else(|| limit("resource count"))?;
            self.budget.url_bytes = self
                .budget
                .url_bytes
                .checked_sub(url.len())
                .ok_or_else(|| limit("destination byte"))?;
            let Some(Target::Local {
                file,
                fragment: Some(fragment),
                ..
            }) = self.resolver.resolve(uri, url)
            else {
                continue;
            };
            let (buffer, path) = match &file {
                Some(file) => match files::open_file_uri(self.open_uris, file, uri) {
                    Some(target) => (Some(target), None),
                    None => (None, Some(file.path.as_path())),
                },
                None => match source_path {
                    Some(path) => (None, Some(path)),
                    None => (Some(uri), None),
                },
            };
            let matches = match (&self.subject.key, buffer, path) {
                (Key::Buffer(expected), Some(actual), _) => expected == actual,
                (Key::File(expected), _, Some(actual)) => expected == actual,
                _ => false,
            };
            let bytes = std::mem::size_of::<ResolvedResource>()
                + fragment.len()
                + buffer.map_or_else(|| path.unwrap().as_os_str().len(), str::len);
            if bytes > MAX_URL_BYTES - cache_bytes {
                cached = None;
            }
            if let Some(cached) = &mut cached {
                cache_bytes += bytes;
                cached.push(ResolvedResource {
                    range,
                    target: buffer.map_or_else(
                        || Key::File(path.unwrap().to_path_buf()),
                        |uri| Key::Buffer(uri.to_string()),
                    ),
                    fragment: fragment.clone().into_boxed_str(),
                });
            }
            if matches && fragment == self.subject.identifier {
                self.push(uri, range)?;
            }
        }
        Ok(cached.map(|entries| CachedResolution {
            resources: before_resources - self.budget.resources,
            url_bytes: before_urls - self.budget.url_bytes,
            entries: entries.into_boxed_slice(),
        }))
    }

    fn push(&mut self, uri: &str, range: Range) -> Result<(), ResponseError> {
        self.budget.locations = self
            .budget
            .locations
            .checked_sub(1)
            .ok_or_else(|| limit("result count"))?;
        self.budget.location_bytes = self
            .budget
            .location_bytes
            .checked_sub(256 + 6 * uri.len())
            .ok_or_else(|| limit("result byte"))?;
        self.locations.push(Location {
            uri: uri.to_string(),
            range,
        });
        Ok(())
    }
}

fn headings<'a>(
    root: &'a Root,
    prefix: &str,
    cancellation: &Cancellation,
) -> Result<impl Iterator<Item = (String, Range)> + 'a, ResponseError> {
    cancellation.check()?;
    if root
        .children
        .iter()
        .filter(|node| matches!(node, Node::Heading(_)))
        .take(20_001)
        .count()
        > 20_000
    {
        return Err(limit("target heading count"));
    }
    Ok(links::headings(root, prefix))
}

fn limit(resource: &str) -> ResponseError {
    ResponseError::new(-32000, format!("heading references exceed the {resource} limit; use narrower workspace roots or close buffers"))
}

fn modified() -> ResponseError {
    ResponseError::new(-32801, "workspace paths changed during reference analysis")
}

#[cfg(test)]
mod tests;
