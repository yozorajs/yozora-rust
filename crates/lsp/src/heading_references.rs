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

const MAX_URL_BYTES: usize = 8 * 1024 * 1024;

pub struct Context<'a> {
    pub workspace: &'a Workspace,
    pub documents: &'a mut HashMap<String, Document>,
    pub parser: &'a YozoraParser,
    pub cancellation: &'a Cancellation,
    pub used_buffers: &'a mut HashSet<String>,
    pub heading_id_prefix: &'a str,
}

enum Selection {
    Heading { identifier: String, range: Range },
    Destination(String),
}

#[derive(Eq, PartialEq)]
enum Key {
    Buffer(String),
    File(PathBuf),
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
            resources: 20_000,
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
        if !local {
            let sources = scope.sources(
                self.documents.keys().map(String::as_str),
                self.cancellation,
                files::MAX_WORKSPACE_ENTRIES,
                files::MAX_WORKSPACE_FILES,
            )?;
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
        let mut search = Search {
            subject: &subject,
            scope: &scope,
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
            search.collect(&uri, root)?;
        }
        for path in paths {
            self.cancellation.check()?;
            if scope.resolve(&path).as_ref() != Some(&path) {
                return Err(modified());
            }
            let mut document = if subject.key == Key::File(path.clone()) {
                disk.take().expect("a closed target has a parsed snapshot")
            } else {
                let Some(text) =
                    files::read_markdown_budgeted(&path, &mut search.budget.text_bytes)?
                else {
                    continue;
                };
                Document::new(0, text)?
            };
            self.cancellation.check()?;
            let root = document.ast(self.parser)?;
            self.cancellation.check()?;
            let uri = files::path_uri(&path).expect("scoped paths form local file URIs");
            search.collect(&uri, root)?;
        }
        if include_declaration {
            search.push(&subject.declaration.uri, subject.declaration.range)?;
        }
        if scope
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
    scope: &'a FileScope,
    open_uris: &'a HashMap<PathBuf, Vec<String>>,
    cancellation: &'a Cancellation,
    budget: Budget,
    locations: Vec<Location>,
}

impl Search<'_> {
    fn collect(&mut self, uri: &str, root: &Root) -> Result<(), ResponseError> {
        self.cancellation.check()?;
        for (range, url) in links::resources(root) {
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
            }) = files::resolve(uri, url, self.scope)
            else {
                continue;
            };
            if fragment != self.subject.identifier {
                continue;
            }
            let matches = match file {
                Some(file) => match files::open_file_uri(self.open_uris, &file, uri) {
                    Some(target) => {
                        matches!(&self.subject.key, Key::Buffer(expected) if expected == target)
                    }
                    None => {
                        matches!(&self.subject.key, Key::File(expected) if expected == &file.path)
                    }
                },
                None => {
                    matches!(&self.subject.key, Key::Buffer(expected) if expected == uri)
                        || matches!(&self.subject.key, Key::File(_) if self.subject.declaration.uri == uri)
                }
            };
            if matches {
                self.push(uri, range)?;
            }
        }
        Ok(())
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
