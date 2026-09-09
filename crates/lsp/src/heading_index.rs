use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::cancellation::Cancellation;
use crate::files::{self, FileScope};
use crate::links;
use crate::protocol::{Range, ResponseError};

type Headings = Arc<[(String, Range)]>;

struct Entry {
    text: Box<str>,
    prefix: String,
    headings: Headings,
    bytes: usize,
}

/// Closed-target definitions and completions share bounded heading summaries.
/// Every lookup resolves scope and reads current bytes, including for clients
/// that do not support file events. Cached ASTs are not retained.
#[derive(Default)]
pub struct Index {
    entries: HashMap<PathBuf, Entry>,
    bytes: usize,
}

impl Index {
    pub fn headings(
        &mut self,
        path: &Path,
        scope: &FileScope,
        prefix: &str,
        parser: &YozoraParser,
        cancellation: &Cancellation,
    ) -> Result<Option<Headings>, ResponseError> {
        cancellation.check()?;
        if scope.resolve(path).as_deref() != Some(path) {
            return Ok(None);
        }
        let text = files::read_markdown(path);
        cancellation.check()?;
        let Some(text) = text else {
            self.remove(path);
            return Ok(None);
        };
        if let Some(entry) = self.entries.get(path) {
            if entry.prefix == prefix && entry.text.as_ref() == text {
                return Ok(Some(Arc::clone(&entry.headings)));
            }
        }
        self.remove(path);
        let root = parser.parse(
            &text,
            Some(ParseOptions {
                should_reserve_position: Some(true),
                ..ParseOptions::default()
            }),
        );
        cancellation.check()?;
        let headings: Headings = links::headings(&root, prefix).collect();
        cancellation.check()?;
        let bytes = std::mem::size_of::<Entry>()
            + path.as_os_str().len()
            + text.len()
            + prefix.len()
            + std::mem::size_of_val(headings.as_ref())
            + headings
                .iter()
                .map(|(identifier, _)| identifier.capacity())
                .sum::<usize>();
        let capacity = 20 * 1024 * 1024;
        if bytes <= capacity {
            if self.bytes + bytes > capacity {
                self.entries.clear();
                self.bytes = 0;
            }
            self.bytes += bytes;
            self.entries.insert(
                path.to_path_buf(),
                Entry {
                    text: text.into_boxed_str(),
                    prefix: prefix.to_string(),
                    headings: Arc::clone(&headings),
                    bytes,
                },
            );
        }
        Ok(Some(headings))
    }

    fn remove(&mut self, path: &Path) {
        if let Some(entry) = self.entries.remove(path) {
            self.bytes -= entry.bytes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::{tests::TestDir, Workspace, MAX_WORKSPACE_ROOTS};

    #[test]
    fn summaries_require_current_contents_scope_and_identifier_prefix() {
        let directory = TestDir::new();
        let path = directory.0.join("target.md");
        let cancellation = Cancellation::default();
        let scope = Workspace::new(vec![directory.uri("")])
            .index_scope(&cancellation, MAX_WORKSPACE_ROOTS)
            .unwrap();
        let parser = YozoraParser::default();
        let mut index = Index::default();
        std::fs::write(&path, "# One\n\n## One").unwrap();
        let first = index
            .headings(&path, &scope, "", &parser, &cancellation)
            .unwrap()
            .unwrap();
        let reused = index
            .headings(&path, &scope, "", &parser, &cancellation)
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&first, &reused));
        let prefixed = index
            .headings(&path, &scope, "h-", &parser, &cancellation)
            .unwrap()
            .unwrap();
        assert_eq!(prefixed[1].0, "h-one-2");
        std::fs::write(&path, "# Two\n\n## Two").unwrap();
        let changed = index
            .headings(&path, &scope, "", &parser, &cancellation)
            .unwrap()
            .unwrap();
        assert_eq!(changed[1].0, "two-2");
        assert_eq!(first[1].0, "one-2");
        let empty = Workspace::default()
            .index_scope(&cancellation, MAX_WORKSPACE_ROOTS)
            .unwrap();
        assert!(index
            .headings(&path, &empty, "", &parser, &cancellation)
            .unwrap()
            .is_none());
        std::fs::remove_file(&path).unwrap();
        assert!(index
            .headings(&path, &scope, "", &parser, &cancellation)
            .unwrap()
            .is_none());
        assert_eq!(index.bytes, 0);
    }
}
