use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use yozora_core_parser::{DefaultParserProps, ParseOptions};

use super::*;
use crate::files::tests::TestDir;

struct Project {
    directory: TestDir,
    workspace: Workspace,
    documents: HashMap<String, Document>,
    parser: YozoraParser,
    used: HashSet<String>,
    prefix: String,
    cancellation: Cancellation,
    index: Index,
    catalog: files::Catalog,
}

impl Project {
    fn new() -> Self {
        let directory = TestDir::new();
        let workspace = Workspace::new(vec![directory.uri("")]);
        Self {
            directory,
            workspace,
            documents: HashMap::new(),
            parser: YozoraParser::default(),
            used: HashSet::new(),
            prefix: String::new(),
            cancellation: Cancellation::default(),
            index: Index::default(),
            catalog: files::Catalog::default(),
        }
    }

    fn open(&mut self, relative: &str, text: &str) -> String {
        let uri = self.directory.uri(relative);
        self.documents
            .insert(uri.clone(), Document::new(1, text.to_string()).unwrap());
        uri
    }

    fn disk(&self, relative: &str, text: &str) {
        let path = self.directory.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn references(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<Location>, ResponseError> {
        Context {
            index: &mut self.index,
            catalog: &mut self.catalog,
            revision: None,
            workspace: &self.workspace,
            documents: &mut self.documents,
            parser: &self.parser,
            cancellation: &self.cancellation,
            used_buffers: &mut self.used,
            heading_id_prefix: &self.prefix,
        }
        .find(uri, Position { line, character }, include_declaration)
    }
}

#[test]
fn cached_disk_references_observe_same_length_edits_and_buffer_overlays() {
    let mut project = Project::new();
    let target = project.open("target.md", "# Target");
    project.disk("incoming.md", "[a](target.md#target)");
    for _ in 0..2 {
        assert_eq!(project.references(&target, 0, 3, false).unwrap().len(), 1);
    }
    project.disk("incoming.md", "[a](target.md#absent)");
    assert!(project.references(&target, 0, 3, false).unwrap().is_empty());
    project.disk("incoming.md", "[a](target.md#target)");
    let incoming = project.open("incoming.md", "No unsaved link");
    assert!(project.references(&target, 0, 3, false).unwrap().is_empty());
    project.documents.remove(&incoming);
    assert_eq!(project.references(&target, 0, 3, false).unwrap().len(), 1);
    fs::remove_file(project.directory.0.join("incoming.md")).unwrap();
    assert!(project.references(&target, 0, 3, false).unwrap().is_empty());
}

#[test]
fn headings_find_direct_images_and_bound_references_with_unsaved_overlays() {
    let mut project = Project::new();
    let target = project.open(
        "target.md",
        "# Intro\n[local](#intro)\n## Intro\n[second](#intro-2)\n",
    );
    project.disk("target.md", "# Stale disk heading");
    project.disk(
        "docs/closed.md",
        "😀 [go](../target.md#intro)\n![image](../target.md#intro)",
    );
    project.disk("labels.md", "[one][r]\n![two][r]\n\n[r]: target.md#intro\n[R]: target.md#intro-2\n\n`[code](target.md#intro)`");
    project.disk("open.md", "[stale](target.md#intro-2)");
    project.open("open.md", "[live](target.md#intro)");
    let scratch = "untitled:scratch";
    project.documents.insert(
        scratch.to_string(),
        Document::new(1, format!("[absolute]({target}#intro)")).unwrap(),
    );

    let locations = project.references(&target, 0, 3, false).unwrap();
    assert_eq!(locations.len(), 8);
    let positions: Vec<_> = locations
        .iter()
        .map(|location| (location.uri.as_str(), location.range.start.line))
        .collect();
    assert_eq!(
        positions,
        vec![
            (project.directory.uri("docs/closed.md").as_str(), 0),
            (project.directory.uri("docs/closed.md").as_str(), 1),
            (project.directory.uri("labels.md").as_str(), 0),
            (project.directory.uri("labels.md").as_str(), 1),
            (project.directory.uri("labels.md").as_str(), 3),
            (project.directory.uri("open.md").as_str(), 0),
            (target.as_str(), 1),
            (scratch, 0),
        ]
    );
    assert_eq!(locations[0].range.start.character, 3);
    let declared = project.references(&target, 0, 3, true).unwrap();
    assert_eq!(declared.len(), locations.len() + 1);
    let declaration = declared
        .iter()
        .find(|location| location.uri == target && location.range.start.line == 0)
        .unwrap();
    assert_eq!(declaration.range.start.character, 2);
    assert_eq!(declaration.range.end.character, 7);
    let second = project.references(&target, 2, 4, false).unwrap();
    assert_eq!(second.len(), 2);
    assert_eq!(
        second[0].range.start.line, 4,
        "inactive definition is a written destination, but owns no occurrences"
    );
    assert_eq!(second[1].range.start.line, 3);
    assert_eq!(
        project.documents[&target].text().unwrap(),
        "# Intro\n[local](#intro)\n## Intro\n[second](#intro-2)\n"
    );
}

#[test]
fn a_direct_link_resolves_a_closed_target_once_and_includes_its_self_references() {
    let mut project = Project::new();
    project.disk("target.md", "# Intro\n[self](#intro)\n[probe](count)");
    project.disk("second.md", "[other](target.md#intro)");
    let uri = project.open("source.md", "[go](target.md#intro)");
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    project.parser = YozoraParser::new(DefaultParserProps {
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
    let locations = project.references(&uri, 0, 2, true).unwrap();
    assert_eq!(locations.len(), 4);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "target was reparsed during the workspace scan"
    );
    assert_eq!(locations[2].uri, project.directory.uri("target.md"));
    assert_eq!(locations[2].range.start.line, 0);
    assert_eq!(locations[3].range.start.line, 1);
    assert!(!project
        .documents
        .contains_key(&project.directory.uri("target.md")));
}

#[test]
fn exact_unicode_ids_prefixes_and_setext_declarations_match_navigation() {
    let mut project = Project::new();
    project.prefix = "h-".to_string();
    let target = project.open("guide 中文.md", "中文😀\r\n====\r\n\r\n> # Nested\r\n");
    let source = project.open("source.md", "😀 [go](guide%20%E4%B8%AD%E6%96%87.md?view=raw#h-%E4%B8%AD%E6%96%87%F0%9F%98%80)\r\n[wrong](guide%20%E4%B8%AD%E6%96%87.md#h-%25E4%25B8%25AD)\r\n[nested](guide%20%E4%B8%AD%E6%96%87.md#h-nested)");
    let locations = project.references(&source, 0, 5, true).unwrap();
    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0].uri, target);
    assert_eq!(
        locations[0].range.start,
        Position {
            line: 0,
            character: 0
        }
    );
    assert_eq!(
        locations[0].range.end,
        Position {
            line: 0,
            character: 4
        }
    );
    assert_eq!(locations[1].range.start.character, 3);
    assert!(project.references(&source, 1, 2, true).unwrap().is_empty());
    assert!(project.references(&source, 2, 2, true).unwrap().is_empty());
    assert!(project.references(&target, 3, 5, true).unwrap().is_empty());
}

#[test]
fn open_aliases_keep_independent_links_and_source_preference() {
    let mut project = Project::new();
    project.disk("guide.md", "# Disk");
    let first = project
        .directory
        .uri("guide.md")
        .replacen("file://", "FILE://localhost", 1);
    let second = project.directory.uri("%67uide.md").replace("%2567", "%67");
    for (uri, text) in [
        (&first, "# Alpha\n[self](guide.md#alpha)"),
        (&second, "# Beta\n[self](guide.md#beta)"),
    ] {
        project
            .documents
            .insert(uri.clone(), Document::new(1, text.to_string()).unwrap());
    }
    project.open(
        "source.md",
        &format!("[a]({first}#alpha)\n[b]({second}#beta)\n[fallback](guide.md#alpha)"),
    );
    let alpha = project.references(&first, 0, 3, false).unwrap();
    assert_eq!(alpha.len(), 3);
    assert!(alpha
        .iter()
        .any(|location| location.uri == first && location.range.start.line == 1));
    assert!(!alpha.iter().any(|location| location.uri == second));
    let beta = project.references(&second, 0, 3, false).unwrap();
    assert_eq!(beta.len(), 2);
    assert!(beta
        .iter()
        .any(|location| location.uri == second && location.range.start.line == 1));
    assert!(project.used.contains(&first) && project.used.contains(&second));
}

#[test]
fn missing_targets_and_non_heading_resources_do_not_start_workspace_searches() {
    let mut project = Project::new();
    project.disk("image.png", "fixture");
    let uri = project.open("source.md", "[missing](absent.md#intro)\n[file](image.png)\n[image](image.png#view)\n[remote](https://example.test/#intro)\n`[code](target.md#intro)`\n[blocked](.ssh/target.md#intro)");
    project.open("out-of-sync.md", "# Unavailable");
    project
        .documents
        .get_mut(&project.directory.uri("out-of-sync.md"))
        .unwrap()
        .invalidate(2)
        .unwrap();
    for line in 0..6 {
        assert!(
            project.references(&uri, line, 2, true).unwrap().is_empty(),
            "line {line}"
        );
    }
    let target = project.open("target.md", "# Intro");
    assert_eq!(
        project.references(&target, 0, 3, true).unwrap_err().code,
        -32801
    );
}

#[test]
fn without_local_roots_only_same_buffer_anchors_are_searched() {
    let mut project = Project::new();
    project.disk("closed.md", "[go](target.md#intro)");
    let uri = project.open("target.md", "# Intro\n[self](#intro)");
    project.open("other.md", "[file](target.md#intro)\n[self](#intro)");
    for roots in [vec![], vec!["vscode-remote://host/docs".to_string()]] {
        project.workspace = Workspace::new(roots);
        let locations = project.references(&uri, 0, 3, false).unwrap();
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, uri);
        assert_eq!(locations[0].range.start.line, 1);
    }
    let scratch = "untitled:heading";
    project
        .documents
        .get_mut(&project.directory.uri("other.md"))
        .unwrap()
        .invalidate(2)
        .unwrap();
    project.documents.insert(
        scratch.to_string(),
        Document::new(1, "# Intro\n[self](#intro)".to_string()).unwrap(),
    );
    assert_eq!(project.references(scratch, 1, 2, true).unwrap().len(), 2);
    assert_eq!(project.references(&uri, 0, 3, true).unwrap().len(), 2);
}

#[test]
fn cancellation_during_disk_analysis_prevents_later_parses() {
    let mut project = Project::new();
    let uri = project.open("target.md", "# Intro");
    project.disk("a.md", "[go](stop)");
    project.disk("b.md", "[go](unexpected)");
    let cancellation = project.cancellation.clone();
    project.parser = YozoraParser::new(DefaultParserProps {
        default_parse_options: Some(ParseOptions {
            format_url: Some(Arc::new(move |url| {
                assert_eq!(url, "stop", "analysis continued after cancellation");
                cancellation.cancel();
                url.to_string()
            })),
            ..ParseOptions::default()
        }),
        ..DefaultParserProps::default()
    });
    for _ in 0..2 {
        assert_eq!(
            project.references(&uri, 0, 3, true).unwrap_err().code,
            -32800
        );
    }
}

#[test]
fn resource_and_result_budgets_reject_partial_reference_lists() {
    let parser = YozoraParser::default();
    let mut document =
        Document::new(1, "# Intro\n[one](#intro) [two](#intro)".to_string()).unwrap();
    let root = document.ast(&parser).unwrap();
    let uri = "untitled:budget";
    let subject = Subject {
        key: Key::Buffer(uri.to_string()),
        identifier: "intro".to_string(),
        declaration: Location {
            uri: uri.to_string(),
            range: Range {
                start: Position::default(),
                end: Position::default(),
            },
        },
    };
    let workspace = Workspace::default();
    let cancellation = Cancellation::default();
    let scope = workspace
        .index_scope(&cancellation, files::MAX_WORKSPACE_ROOTS)
        .unwrap();
    for budget in [
        Budget {
            resources: 1,
            ..Budget::default()
        },
        Budget {
            url_bytes: 6,
            ..Budget::default()
        },
        Budget {
            locations: 1,
            ..Budget::default()
        },
        Budget {
            location_bytes: 256 + 6 * uri.len(),
            ..Budget::default()
        },
    ] {
        let mut search = Search {
            subject: &subject,
            resolver: files::Resolver::new(&scope),
            open_uris: &HashMap::new(),
            cancellation: &cancellation,
            budget,
            locations: Vec::new(),
        };
        assert_eq!(search.collect(uri, root, None).unwrap_err().code, -32000);
    }
}
