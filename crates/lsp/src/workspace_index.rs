use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

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
    range: Range,
    // Indices avoid duplicating a long parent name for every child in the cache.
    parent: Option<usize>,
}

struct CachedFile {
    text: Box<str>,
    headings: Box<[Heading]>,
    summary_bytes: usize,
}

impl CachedFile {
    fn owned_bytes(&self, path: &PathBuf) -> usize {
        std::mem::size_of::<Self>() + path.capacity() + self.text.len() + self.summary_bytes
    }
}

/// Each worker owns a bounded cache of disk text and heading summaries, never
/// disk ASTs. Every query rediscovers files and compares their current contents;
/// no cached entry is a source of truth for existence, scope, or buffer identity.
#[derive(Default)]
pub struct Index {
    cache: BTreeMap<PathBuf, CachedFile>,
    cache_bytes: usize,
    limits: Limits,
}

#[derive(Default)]
struct Budget {
    text_bytes: usize,
    summary_bytes: usize,
    result_bytes: usize,
}

impl Index {
    pub fn symbols(
        &mut self,
        query: &str,
        workspace: &Workspace,
        documents: &mut HashMap<String, Document>,
        parser: &YozoraParser,
        cancellation: &Cancellation,
        used_buffers: &mut HashSet<String>,
    ) -> Result<Vec<SymbolInformation>, ResponseError> {
        cancellation.check()?;
        let scope = workspace.index_scope(cancellation, self.limits.roots)?;
        let sources = scope.sources(
            documents.keys().map(String::as_str),
            cancellation,
            self.limits.entries,
            self.limits.files,
        )?;
        let paths = sources.files;
        let buffers = sources.buffers.into_iter().map(|buffer| buffer.uri);
        let retained: BTreeSet<_> = paths.iter().collect();
        self.cache.retain(|path, _| retained.contains(path));
        self.cache_bytes = self
            .cache
            .iter()
            .map(|(path, entry)| entry.owned_bytes(path))
            .sum();

        let query = query.trim().to_lowercase();
        let mut budget = Budget::default();
        let mut result = Vec::new();
        for uri in buffers {
            cancellation.check()?;
            used_buffers.insert(uri.clone());
            let document = documents.get_mut(&uri).expect("selected buffer is open");
            budget.text_bytes += document.text()?.len();
            self.check_text_budget(&budget)?;
            cancellation.check()?;
            let root = document.ast(parser)?;
            let (headings, bytes) = summarize(
                root,
                self.limits.summary_bytes - budget.summary_bytes,
                cancellation,
            )?;
            budget.summary_bytes += bytes;
            self.collect(
                &headings,
                &uri,
                &query,
                &mut budget,
                &mut result,
                cancellation,
            )?;
        }
        for path in paths {
            cancellation.check()?;
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
            let uri = files::path_uri(&path).expect("scoped file paths have valid URIs");
            let collected = self.collect(
                &entry.headings,
                &uri,
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
        if self.cache_bytes + bytes > self.limits.cache_bytes {
            // Old, not-yet-visited contents may be larger than their replacements.
            // Dropping the cache only affects reuse; results always use this scan.
            self.cache.clear();
            self.cache_bytes = 0;
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
            if !heading.name.to_lowercase().contains(query) {
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
        bytes += std::mem::size_of::<Heading>() + symbol.name.capacity();
        if bytes > max_bytes {
            return Err(index_limit("heading summary byte"));
        }
        let index = headings.len();
        headings.push(Heading {
            name: symbol.name,
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
