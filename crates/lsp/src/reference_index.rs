use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::cancellation::Cancellation;
use crate::links;
use crate::protocol::{Range, ResponseError};

pub const MAX_RESOURCES: usize = 20_000;
pub const MAX_URL_BYTES: usize = 8 * 1024 * 1024;

pub struct Resource {
    pub range: Range,
    pub url: Box<str>,
}

#[derive(Clone, Eq, PartialEq)]
pub enum Key {
    Buffer(String),
    File(PathBuf),
}

pub struct ResolvedResource {
    pub range: Range,
    pub target: Key,
    pub fragment: Box<str>,
}

pub struct Resolved {
    pub resources: usize,
    pub url_bytes: usize,
    pub entries: Box<[ResolvedResource]>,
}

impl Resolved {
    fn owned_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + std::mem::size_of_val(self.entries.as_ref())
            + self
                .entries
                .iter()
                .map(|entry| {
                    entry.fragment.len()
                        + match &entry.target {
                            Key::Buffer(uri) => uri.capacity(),
                            Key::File(path) => path.capacity(),
                        }
                })
                .sum::<usize>()
    }
}

struct Entry {
    uri: Box<str>,
    text: Box<str>,
    resources: Arc<[Resource]>,
    bytes: usize,
    resolved: Option<(u64, u64, Arc<Resolved>)>,
}

/// Parser-local semantic summaries. Callers validate file-event revisions or
/// current bytes before reusing reference bindings and source ranges.
pub struct Index {
    entries: HashMap<PathBuf, Entry>,
    bytes: usize,
    capacity: usize,
    revision: Option<u64>,
    catalog: u64,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            capacity: 40 * 1024 * 1024,
            revision: None,
            catalog: 0,
        }
    }
}

impl Index {
    pub fn resolved(
        &self,
        path: &Path,
        revision: u64,
        catalog: &crate::files::Catalog,
    ) -> Option<(usize, &str, &Resolved)> {
        let entry = self.entries.get(path)?;
        let (stored_revision, stored_catalog, resolved) = entry.resolved.as_ref()?;
        (*stored_catalog == catalog.generation()
            && catalog.contents_unchanged(path, Some(*stored_revision), Some(revision)))
        .then_some((entry.text.len(), entry.uri.as_ref(), resolved.as_ref()))
    }

    pub fn cache_resolved(&mut self, path: &Path, revision: u64, catalog: u64, resolved: Resolved) {
        let Some(entry) = self.entries.get_mut(path) else {
            return;
        };
        if let Some((_, _, old)) = entry.resolved.take() {
            let bytes = old.owned_bytes();
            self.bytes -= bytes;
            entry.bytes -= bytes;
        }
        let bytes = resolved.owned_bytes();
        if bytes <= self.capacity - self.bytes {
            self.bytes += bytes;
            entry.bytes += bytes;
            entry.resolved = Some((revision, catalog, Arc::new(resolved)));
        }
    }

    pub fn revision(&self, catalog: u64) -> Option<u64> {
        (self.catalog == catalog).then_some(self.revision).flatten()
    }

    pub fn finish_revision(&mut self, revision: Option<u64>, catalog: u64) {
        self.revision = revision;
        self.catalog = catalog;
    }

    pub fn cached(&self, path: &Path) -> Option<(usize, Arc<[Resource]>)> {
        self.entries
            .get(path)
            .map(|entry| (entry.text.len(), Arc::clone(&entry.resources)))
    }

    pub fn retain(&mut self, paths: &BTreeSet<PathBuf>) {
        self.entries.retain(|path, _| paths.contains(path));
        self.bytes = self.entries.values().map(|entry| entry.bytes).sum();
    }

    pub fn remove(&mut self, path: &Path) {
        if let Some(entry) = self.entries.remove(path) {
            self.bytes -= entry.bytes;
        }
    }

    pub fn resources(
        &mut self,
        path: &Path,
        text: String,
        parser: &YozoraParser,
        cancellation: &Cancellation,
    ) -> Result<Arc<[Resource]>, ResponseError> {
        cancellation.check()?;
        if let Some(entry) = self.entries.get(path) {
            if entry.text.as_ref() == text {
                return Ok(Arc::clone(&entry.resources));
            }
        }
        let root = parser.parse(
            &text,
            Some(ParseOptions {
                should_reserve_position: Some(true),
                ..ParseOptions::default()
            }),
        );
        cancellation.check()?;
        let mut resources = Vec::new();
        let mut url_bytes = 0;
        for (range, url) in links::resources(&root) {
            cancellation.check()?;
            if resources.len() == MAX_RESOURCES {
                return Err(limit("resource count"));
            }
            if url.len() > MAX_URL_BYTES - url_bytes {
                return Err(limit("destination byte"));
            }
            url_bytes += url.len();
            resources.push(Resource {
                range,
                url: url.into(),
            });
        }
        let resources: Arc<[Resource]> = resources.into();
        let path = path.to_path_buf();
        let uri = crate::files::path_uri(&path)
            .expect("scoped paths form file URIs")
            .into_boxed_str();
        let bytes = std::mem::size_of::<Entry>()
            + path.capacity()
            + uri.len()
            + text.len()
            + std::mem::size_of_val(resources.as_ref())
            + url_bytes;
        self.remove(&path);
        if bytes <= self.capacity {
            if self.bytes + bytes > self.capacity {
                self.entries.clear();
                self.bytes = 0;
            }
            self.bytes += bytes;
            self.entries.insert(
                path,
                Entry {
                    uri,
                    text: text.into_boxed_str(),
                    resources: Arc::clone(&resources),
                    bytes,
                    resolved: None,
                },
            );
        }
        Ok(resources)
    }
}

fn limit(resource: &str) -> ResponseError {
    ResponseError::new(-32000, format!("heading references exceed the {resource} limit; use narrower workspace roots or close buffers"))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use yozora_core_parser::DefaultParserProps;

    use super::*;

    #[test]
    fn reuse_requires_identical_bytes_and_eviction_does_not_change_results() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        let parser = YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    counted.fetch_add(1, Ordering::Relaxed);
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        });
        let cancellation = Cancellation::default();
        let path = Path::new("/a.md");
        let mut index = Index::default();
        let first = index
            .resources(path, "[a](#one)".into(), &parser, &cancellation)
            .unwrap();
        let same = index
            .resources(path, "[a](#one)".into(), &parser, &cancellation)
            .unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let changed = index
            .resources(path, "[a](#two)".into(), &parser, &cancellation)
            .unwrap();
        assert_eq!(changed[0].url.as_ref(), "#two");
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(first[0].url.as_ref(), "#one");

        index.capacity = 1;
        index.remove(path);
        for _ in 0..2 {
            let uncached = index
                .resources(path, "[a](#two)".into(), &parser, &cancellation)
                .unwrap();
            assert_eq!(uncached[0].url.as_ref(), "#two");
            assert!(index.bytes <= index.capacity);
            assert!(index.entries.is_empty());
        }
    }
}
