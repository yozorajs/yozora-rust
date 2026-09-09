use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use yozora_ast::Root;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::analysis;
use crate::cancellation::Cancellation;
use crate::document::Document;
use crate::files::{self, Workspace};
use crate::protocol::{Location, Range, ResponseError, SymbolInformation};

#[derive(Clone, Copy)]
struct Limits {
    roots: usize,
    entries: usize,
    files: usize,
    text_bytes: usize,
    summary_bytes: usize,
    cache_bytes: usize,
    results: usize,
    result_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            roots: files::MAX_WORKSPACE_ROOTS,
            entries: files::MAX_WORKSPACE_ENTRIES,
            files: files::MAX_WORKSPACE_FILES,
            text_bytes: files::MAX_WORKSPACE_BYTES,
            summary_bytes: 8 * 1024 * 1024,
            cache_bytes: 40 * 1024 * 1024,
            results: 1_000,
            result_bytes: 1024 * 1024,
        }
    }
}

struct Heading {
    name: String,
    folded: String,
    range: Range,
    // Indices avoid duplicating a long parent name for every child in the cache.
    parent: Option<usize>,
}

struct CachedFile {
    uri: String,
    text: Box<str>,
    headings: Box<[Heading]>,
    summary_bytes: usize,
}

struct CachedBuffer {
    text: Arc<String>,
    headings: Box<[Heading]>,
    summary_bytes: usize,
}

impl CachedBuffer {
    fn owned_bytes(&self, uri: &String) -> usize {
        std::mem::size_of::<Self>() + uri.capacity() + self.text.capacity() + self.summary_bytes
    }
}

impl CachedFile {
    fn owned_bytes(&self, path: &PathBuf) -> usize {
        std::mem::size_of::<Self>()
            + path.capacity()
            + self.uri.capacity()
            + self.text.len()
            + self.summary_bytes
    }
}

/// Each worker owns bounded semantic caches, never closed ASTs. Confirmed file
/// events validate unchanged sources; strict clients compare current disk bytes.
/// Catalog generations and open text identities delimit each provider's reuse.
#[derive(Default)]
pub struct Index {
    pub references: crate::reference_index::Index,
    pub refactors: crate::refactor::Index,
    pub targets: crate::heading_index::Index,
    pub catalog: files::Catalog,
    symbol_revision: Option<u64>,
    symbol_catalog: u64,
    cache: HashMap<PathBuf, CachedFile>,
    cache_bytes: usize,
    buffers: HashMap<String, CachedBuffer>,
    buffer_bytes: usize,
    limits: Limits,
}

#[derive(Default)]
struct Budget {
    text_bytes: usize,
    summary_bytes: usize,
    result_bytes: usize,
}

impl Index {
    #[allow(clippy::too_many_arguments)]
    pub fn symbols(
        &mut self,
        query: &str,
        workspace: &Workspace,
        documents: &mut HashMap<String, Document>,
        parser: &YozoraParser,
        cancellation: &Cancellation,
        used_buffers: &mut HashSet<String>,
        revision: Option<u64>,
    ) -> Result<Vec<SymbolInformation>, ResponseError> {
        cancellation.check()?;
        let scope = workspace.index_scope(cancellation, self.limits.roots)?;
        let (sources, reused) = self.catalog.sources(
            &scope,
            documents.keys().map(String::as_str),
            cancellation,
            self.limits.entries,
            self.limits.files,
            revision,
        )?;
        let generation = self.catalog.generation();
        let previous_revision = self.symbol_revision;
        let reuse_contents = reused
            && revision.is_some()
            && previous_revision.is_some()
            && self.symbol_catalog == generation;
        self.symbol_revision = None;
        let paths = sources.files;
        let buffers = sources.buffers.into_iter().map(|buffer| buffer.uri);
        self.buffers.retain(|uri, _| documents.contains_key(uri));
        self.buffer_bytes = self
            .buffers
            .iter()
            .map(|(uri, entry)| entry.owned_bytes(uri))
            .sum();
        if !reuse_contents {
            let retained: HashSet<_> = paths.iter().collect();
            self.cache.retain(|path, _| retained.contains(path));
            self.cache_bytes = self
                .cache
                .iter()
                .map(|(path, entry)| entry.owned_bytes(path))
                .sum();
        }

        let query = query.trim().to_lowercase();
        let mut budget = Budget::default();
        let mut result = Vec::new();
        for uri in buffers {
            cancellation.check()?;
            used_buffers.insert(uri.clone());
            let document = documents.get_mut(&uri).expect("selected buffer is open");
            let text = document.shared_text()?;
            budget.text_bytes += text.len();
            self.check_text_budget(&budget)?;
            cancellation.check()?;
            if let Some(entry) = self
                .buffers
                .get(&uri)
                .filter(|entry| Arc::ptr_eq(&entry.text, &text))
            {
                budget.summary_bytes += entry.summary_bytes;
                if budget.summary_bytes > self.limits.summary_bytes {
                    return Err(index_limit("heading summary byte"));
                }
                self.collect(
                    &entry.headings,
                    &uri,
                    &query,
                    &mut budget,
                    &mut result,
                    cancellation,
                )?;
                continue;
            }
            if let Some((key, entry)) = self.buffers.remove_entry(&uri) {
                self.buffer_bytes -= entry.owned_bytes(&key);
            }
            let root = document.ast(parser)?;
            let (headings, bytes) = summarize(
                root,
                self.limits.summary_bytes - budget.summary_bytes,
                cancellation,
            )?;
            budget.summary_bytes += bytes;
            let collected = self.collect(
                &headings,
                &uri,
                &query,
                &mut budget,
                &mut result,
                cancellation,
            );
            let entry = CachedBuffer {
                text,
                headings,
                summary_bytes: bytes,
            };
            let bytes = entry.owned_bytes(&uri);
            if bytes <= self.limits.cache_bytes {
                if self.cache_bytes + self.buffer_bytes + bytes > self.limits.cache_bytes {
                    self.cache.clear();
                    self.cache_bytes = 0;
                    self.buffers.clear();
                    self.buffer_bytes = 0;
                }
                self.buffer_bytes += bytes;
                self.buffers.insert(uri, entry);
            }
            collected?;
        }
        for path in paths {
            cancellation.check()?;
            if reuse_contents
                && self
                    .catalog
                    .contents_unchanged(&path, previous_revision, revision)
            {
                if let Some(entry) = self.cache.get(&path) {
                    budget.text_bytes += entry.text.len();
                    self.check_text_budget(&budget)?;
                    budget.summary_bytes += entry.summary_bytes;
                    if budget.summary_bytes > self.limits.summary_bytes {
                        return Err(index_limit("heading summary byte"));
                    }
                    self.collect(
                        &entry.headings,
                        &entry.uri,
                        &query,
                        &mut budget,
                        &mut result,
                        cancellation,
                    )?;
                    continue;
                }
            }
            let cached = self.cache.remove_entry(&path).map(|(key, entry)| {
                self.cache_bytes -= entry.owned_bytes(&key);
                entry
            });
            // Discovery can precede a slow parse. A replaced path must be
            // rediscovered on the next query, never read through a new alias
            // that could escape the roots or bypass an unsaved target buffer.
            if scope.resolve(&path).as_ref() != Some(&path) {
                continue;
            }
            let mut remaining = self.limits.text_bytes - budget.text_bytes;
            let text = files::read_markdown_budgeted(&path, &mut remaining)?;
            budget.text_bytes = self.limits.text_bytes - remaining;
            let Some(text) = text else {
                continue;
            };
            cancellation.check()?;
            let entry = match cached {
                Some(entry) if entry.text.as_ref() == text => entry,
                _ => {
                    let root = parser.parse(
                        &text,
                        Some(ParseOptions {
                            should_reserve_position: Some(true),
                            ..ParseOptions::default()
                        }),
                    );
                    let (headings, summary_bytes) = summarize(
                        &root,
                        self.limits.summary_bytes - budget.summary_bytes,
                        cancellation,
                    )?;
                    CachedFile {
                        uri: files::path_uri(&path).expect("scoped file paths have valid URIs"),
                        text: text.into_boxed_str(),
                        headings,
                        summary_bytes,
                    }
                }
            };
            budget.summary_bytes += entry.summary_bytes;
            if budget.summary_bytes > self.limits.summary_bytes {
                return Err(index_limit("heading summary byte"));
            }
            let collected = self.collect(
                &entry.headings,
                &entry.uri,
                &query,
                &mut budget,
                &mut result,
                cancellation,
            );
            self.cache_file(path, entry);
            collected?;
        }
        cancellation.check()?;
        result.sort_by(|left, right| {
            left.location
                .uri
                .cmp(&right.location.uri)
                .then_with(|| left.location.range.start.cmp(&right.location.range.start))
                .then_with(|| left.name.cmp(&right.name))
        });
        self.symbol_revision = revision;
        self.symbol_catalog = generation;
        Ok(result)
    }

    fn check_text_budget(&self, budget: &Budget) -> Result<(), ResponseError> {
        if budget.text_bytes > self.limits.text_bytes {
            Err(index_limit("text byte"))
        } else {
            Ok(())
        }
    }

    fn cache_file(&mut self, path: PathBuf, entry: CachedFile) {
        let bytes = entry.owned_bytes(&path);
        if bytes > self.limits.cache_bytes {
            return;
        }
        if self.cache_bytes + self.buffer_bytes + bytes > self.limits.cache_bytes {
            // Old, not-yet-visited contents may be larger than their replacements.
            // Dropping the cache only affects reuse; results always use this scan.
            self.cache.clear();
            self.cache_bytes = 0;
            self.buffers.clear();
            self.buffer_bytes = 0;
        }
        self.cache_bytes += bytes;
        self.cache.insert(path, entry);
    }

    fn collect(
        &self,
        headings: &[Heading],
        uri: &str,
        query: &str,
        budget: &mut Budget,
        result: &mut Vec<SymbolInformation>,
        cancellation: &Cancellation,
    ) -> Result<(), ResponseError> {
        for heading in headings {
            cancellation.check()?;
            if !heading.folded.contains(query) {
                continue;
            }
            let parent = heading.parent.map(|index| headings[index].name.as_str());
            // Bound owned result strings and their worst-case JSON escaping.
            let bytes = 256 + 6 * (heading.name.len() + uri.len() + parent.map_or(0, str::len));
            if result.len() == self.limits.results
                || bytes > self.limits.result_bytes - budget.result_bytes
            {
                return Err(ResponseError::new(
                    -32000,
                    "workspace symbol results exceed the response limit; narrow the query",
                ));
            }
            budget.result_bytes += bytes;
            result.push(SymbolInformation {
                name: heading.name.clone(),
                kind: 15, // SymbolKind.String, matching document outlines.
                location: Location {
                    uri: uri.to_string(),
                    range: heading.range,
                },
                container_name: parent.map(str::to_string),
            });
        }
        Ok(())
    }
}

fn summarize(
    root: &Root,
    max_bytes: usize,
    cancellation: &Cancellation,
) -> Result<(Box<[Heading]>, usize), ResponseError> {
    cancellation.check()?;
    let symbols = analysis::document_symbols(root);
    let mut stack: Vec<_> = symbols
        .into_iter()
        .rev()
        .map(|symbol| (symbol, None))
        .collect();
    let mut headings = Vec::new();
    let mut bytes = 0;
    while let Some((symbol, parent)) = stack.pop() {
        cancellation.check()?;
        let folded = symbol.name.to_lowercase();
        bytes += std::mem::size_of::<Heading>() + symbol.name.capacity() + folded.capacity();
        if bytes > max_bytes {
            return Err(index_limit("heading summary byte"));
        }
        let index = headings.len();
        headings.push(Heading {
            name: symbol.name,
            folded,
            range: symbol.selection_range,
            parent,
        });
        stack.extend(
            symbol
                .children
                .into_iter()
                .rev()
                .map(|child| (child, Some(index))),
        );
    }
    Ok((headings.into_boxed_slice(), bytes))
}

fn index_limit(resource: &str) -> ResponseError {
    ResponseError::new(-32000, format!("workspace index exceeds the {resource} limit; use narrower workspace roots or close buffers"))
}

#[cfg(test)]
mod tests;
