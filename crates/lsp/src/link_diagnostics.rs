use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{json, Value};
use yozora_ast::{Node, Root};
use yozora_ast_util::calc_heading_identifiers;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::analysis::nodes;
use crate::cancellation::Cancellation;
use crate::document::Document;
use crate::files::{self, FileScope, Target};
use crate::protocol::{Range, ResponseError};

pub struct Resource {
    range: Range,
    url: String,
}

/// Diagnose written destinations once, including declarations used by references.
/// Parser fallback text, code and HTML do not become link errors.
pub fn resources(root: &Root, cancellation: &Cancellation) -> Result<Vec<Resource>, ResponseError> {
    let mut result = Vec::new();
    let mut remaining = 1024 * 1024;
    for node in nodes(&root.children) {
        cancellation.check()?;
        let url = match node {
            Node::Link(node) => &node.url,
            Node::Image(node) => &node.url,
            Node::Definition(node) => &node.url,
            _ => continue,
        };
        if result.len() == 1_000 || url.len() > remaining {
            break;
        }
        remaining -= url.len();
        if let Some(position) = node.position() {
            result.push(Resource {
                range: position.into(),
                url: url.clone(),
            });
        }
    }
    Ok(result)
}

pub struct Context<'a> {
    pub source_uri: &'a str,
    pub documents: &'a mut HashMap<String, Document>,
    pub scope: &'a FileScope,
    pub parser: &'a YozoraParser,
    pub cancellation: &'a Cancellation,
    pub heading_id_prefix: &'a str,
    pub used_buffers: &'a mut HashSet<String>,
    pub uses_files: &'a mut bool,
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum Key {
    Buffer(String),
    File(PathBuf),
}

enum Status {
    Missing,
    Unknown,
    Present(HashSet<String>),
}

impl Context<'_> {
    pub fn inspect(&mut self, resources: Vec<Resource>) -> Result<Vec<Value>, ResponseError> {
        let mut groups: BTreeMap<Key, Vec<(Resource, Option<String>)>> = BTreeMap::new();
        let mut open_uris = None;
        for resource in resources {
            self.cancellation.check()?;
            let target = files::resolve(self.source_uri, &resource.url, self.scope);
            let (file, fragment) = match target {
                Some(Target::External(_)) => continue,
                Some(Target::Local { file, fragment, .. }) => {
                    (file, fragment.filter(|fragment| !fragment.is_empty()))
                }
                None => {
                    let path = resource.url.split(['?', '#']).next().unwrap_or_default();
                    *self.uses_files |=
                        !path.is_empty() && files::completion_path_start(path).is_some();
                    continue;
                }
            };
            let key = if let Some(file) = file {
                *self.uses_files = true;
                if open_uris.is_none() {
                    open_uris = Some(self.scope.open_file_uris(
                        self.documents.keys().map(String::as_str),
                        self.cancellation,
                    )?);
                }
                match files::open_file_uri(open_uris.as_ref().unwrap(), &file, self.source_uri) {
                    Some(uri) => Key::Buffer(uri.to_string()),
                    None => Key::File(file.path),
                }
            } else if fragment.is_some() {
                Key::Buffer(self.source_uri.to_string())
            } else {
                continue;
            };
            groups.entry(key).or_default().push((resource, fragment));
        }
        let mut result = Vec::new();
        let mut remaining = files::MAX_WORKSPACE_BYTES;
        for (key, group) in groups {
            self.cancellation.check()?;
            let wanted: HashSet<_> = group
                .iter()
                .filter_map(|(_, fragment)| fragment.as_deref())
                .collect();
            let status = self.target(&key, &wanted, &mut remaining)?;
            for (resource, fragment) in group {
                let issue = match &status {
                    Status::Missing => Some((
                        "missing-file",
                        "Local link target does not exist.".to_string(),
                    )),
                    Status::Present(found) => fragment
                        .filter(|fragment| !found.contains(fragment))
                        .map(|fragment| {
                            let fragment: String = fragment.chars().take(200).collect();
                            (
                                "missing-anchor",
                                format!("Heading anchor \"{fragment}\" was not found."),
                            )
                        }),
                    Status::Unknown => None,
                };
                if let Some((code, message)) = issue {
                    result.push(json!({ "range": resource.range, "severity": 2, "code": code, "source": "yozora-lsp", "message": message }));
                }
            }
        }
        self.cancellation.check()?;
        Ok(result)
    }

    fn target(
        &mut self,
        key: &Key,
        wanted: &HashSet<&str>,
        remaining: &mut usize,
    ) -> Result<Status, ResponseError> {
        match key {
            Key::Buffer(uri) => {
                if wanted.is_empty() {
                    return Ok(Status::Present(HashSet::new()));
                }
                self.used_buffers.insert(uri.clone());
                let document = self
                    .documents
                    .get_mut(uri)
                    .expect("selected target buffer is open");
                let Ok(text) = document.text() else {
                    return Ok(Status::Unknown);
                };
                if text.len() > *remaining {
                    return Ok(Status::Unknown);
                }
                *remaining -= text.len();
                self.cancellation.check()?;
                let root = document.ast(self.parser)?;
                self.cancellation.check()?;
                anchors(root, wanted, self.heading_id_prefix, self.cancellation)
            }
            Key::File(path) => {
                if self.scope.resolve(path).as_ref() != Some(path) {
                    return Ok(Status::Unknown);
                }
                match path.try_exists() {
                    Ok(false) => return Ok(Status::Missing),
                    Err(_) => return Ok(Status::Unknown),
                    Ok(true) => {}
                }
                if wanted.is_empty() {
                    return Ok(Status::Present(HashSet::new()));
                }
                if !files::is_markdown(path) {
                    return Ok(Status::Unknown);
                }
                let Some(text) = files::read_markdown_budgeted(path, remaining)
                    .ok()
                    .flatten()
                else {
                    return Ok(Status::Unknown);
                };
                self.cancellation.check()?;
                let root = self.parser.parse(
                    text,
                    Some(ParseOptions {
                        should_reserve_position: Some(true),
                        ..ParseOptions::default()
                    }),
                );
                self.cancellation.check()?;
                anchors(&root, wanted, self.heading_id_prefix, self.cancellation)
            }
        }
    }
}

fn anchors(
    root: &Root,
    wanted: &HashSet<&str>,
    prefix: &str,
    cancellation: &Cancellation,
) -> Result<Status, ResponseError> {
    if root
        .children
        .iter()
        .filter(|node| matches!(node, Node::Heading(_)))
        .take(20_001)
        .count()
        > 20_000
    {
        return Ok(Status::Unknown);
    }
    let found = calc_heading_identifiers(root, prefix)
        .into_iter()
        .filter(|identifier| wanted.contains(identifier.as_str()))
        .collect();
    cancellation.check()?;
    Ok(Status::Present(found))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use yozora_core_parser::DefaultParserProps;

    use super::*;
    use crate::files::{tests::TestDir, Workspace};

    fn inspect(
        directory: &TestDir,
        documents: &mut HashMap<String, Document>,
        parser: &YozoraParser,
    ) -> (Vec<Value>, HashSet<String>, bool) {
        let uri = directory.uri("source.md");
        let cancellation = Cancellation::default();
        let input = resources(
            documents.get_mut(&uri).unwrap().ast(parser).unwrap(),
            &cancellation,
        )
        .unwrap();
        let scope = Workspace::new(vec![directory.uri("")]).scope(&uri);
        let mut used = HashSet::new();
        let mut files = false;
        let result = Context {
            source_uri: &uri,
            documents,
            scope: &scope,
            parser,
            cancellation: &cancellation,
            heading_id_prefix: "",
            used_buffers: &mut used,
            uses_files: &mut files,
        }
        .inspect(input)
        .unwrap();
        (result, used, files)
    }

    #[test]
    fn reports_missing_local_targets_and_exact_top_level_anchors() {
        let directory = TestDir::new();
        fs::write(
            directory.0.join("guide.md"),
            "# Intro\n# Intro\n\n> # Nested",
        )
        .unwrap();
        fs::write(directory.0.join("image.png"), "fixture").unwrap();
        let text = "# Local\n\n😀 [missing](missing.md)\n[good](guide.md#intro-2)\n[bad](guide.md#INTRO)\n[nested](guide.md#nested)\n![image](image.png#view)\n[external](https://example.test/missing)\n[blocked](.ssh/secret.md)\n[self](#local)\n[absent](#absent)\n`[code](missing.md)`\n[unknown][label]\n";
        let mut documents = HashMap::from([(
            directory.uri("source.md"),
            Document::new(1, text.to_string()).unwrap(),
        )]);
        let (mut issues, used, files) =
            inspect(&directory, &mut documents, &YozoraParser::default());
        issues.sort_by_key(|issue| issue["range"]["start"]["line"].as_u64());
        assert_eq!(issues.len(), 4);
        assert_eq!(issues[0]["code"], "missing-file");
        assert_eq!(
            issues[0]["range"]["start"],
            json!({ "line": 2, "character": 3 })
        );
        assert!(issues[1..]
            .iter()
            .all(|issue| issue["code"] == "missing-anchor"));
        assert!(used.contains(&directory.uri("source.md")));
        assert!(files);
    }

    #[test]
    fn written_definitions_are_diagnosed_once_and_resources_are_bounded() {
        let directory = TestDir::new();
        let text = format!("{}\n[ref]: missing.md", "[go][ref]\n".repeat(2_000));
        let mut documents =
            HashMap::from([(directory.uri("source.md"), Document::new(1, text).unwrap())]);
        assert_eq!(
            inspect(&directory, &mut documents, &YozoraParser::default())
                .0
                .len(),
            1
        );
        documents.insert(
            directory.uri("source.md"),
            Document::new(2, "[go](missing.md)\n".repeat(1_010)).unwrap(),
        );
        assert_eq!(
            inspect(&directory, &mut documents, &YozoraParser::default())
                .0
                .len(),
            1_000
        );
    }

    #[test]
    fn open_targets_override_disk_and_unknown_text_does_not_produce_anchor_errors() {
        let directory = TestDir::new();
        fs::write(directory.0.join("guide.md"), "# Disk").unwrap();
        let target = directory.uri("guide.md");
        let mut documents = HashMap::from([
            (
                directory.uri("source.md"),
                Document::new(1, "[live](guide.md#live) [disk](guide.md#disk)".to_string())
                    .unwrap(),
            ),
            (
                target.clone(),
                Document::new(1, "# Live".to_string()).unwrap(),
            ),
        ]);
        let (issues, used, _) = inspect(&directory, &mut documents, &YozoraParser::default());
        assert_eq!(issues.len(), 1);
        assert!(issues[0]["message"].as_str().unwrap().contains("disk"));
        assert!(used.contains(&target));
        documents.get_mut(&target).unwrap().invalidate(2).unwrap();
        let (issues, used, _) = inspect(&directory, &mut documents, &YozoraParser::default());
        assert!(issues.is_empty());
        assert!(used.contains(&target));
        fs::write(directory.0.join("guide.md"), [0xff]).unwrap();
        documents.remove(&target);
        assert!(
            inspect(&directory, &mut documents, &YozoraParser::default())
                .0
                .is_empty()
        );
    }

    #[test]
    fn repeated_fragments_share_one_target_parse() {
        let directory = TestDir::new();
        fs::write(directory.0.join("guide.md"), "# Intro\n\n[probe](count)").unwrap();
        let mut documents = HashMap::from([(
            directory.uri("source.md"),
            Document::new(
                1,
                "[good](guide.md#intro) [bad](guide.md#missing)\n".repeat(100),
            )
            .unwrap(),
        )]);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        let parser = YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    if url == "count" {
                        count.fetch_add(1, Ordering::SeqCst);
                    }
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        });
        assert_eq!(inspect(&directory, &mut documents, &parser).0.len(), 100);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn exact_buffer_aliases_have_independent_heading_sets() {
        let directory = TestDir::new();
        let canonical = directory.uri("guide.md");
        let alias = canonical.replacen("file://", "FILE://localhost", 1);
        let text = format!("[a]({alias}#alias) [b](guide.md#canonical) [c](guide.md#alias)");
        let mut documents = HashMap::from([
            (directory.uri("source.md"), Document::new(1, text).unwrap()),
            (
                canonical.clone(),
                Document::new(1, "# Canonical".to_string()).unwrap(),
            ),
            (
                alias.clone(),
                Document::new(1, "# Alias".to_string()).unwrap(),
            ),
        ]);
        let (issues, used, _) = inspect(&directory, &mut documents, &YozoraParser::default());
        assert_eq!(issues.len(), 1);
        assert!(used.contains(&canonical) && used.contains(&alias));
    }
}
