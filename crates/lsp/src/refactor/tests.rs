use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use yozora_core_parser::{DefaultParserProps, ParseOptions};

use super::*;
use crate::files::tests::TestDir;
use crate::protocol::ContentChange;

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
    revision: Option<u64>,
    validate_disk: bool,
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
            revision: None,
            validate_disk: true,
        }
    }

    fn disk(&self, name: &str, text: &str) {
        let path = self.directory.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn open(&mut self, name: &str, text: &str) -> String {
        let uri = self.directory.uri(name);
        self.documents
            .insert(uri.clone(), Document::new(1, text.to_string()).unwrap());
        uri
    }

    fn context(&mut self) -> Context<'_> {
        Context {
            validate_disk: self.validate_disk,
            index: &mut self.index,
            catalog: &mut self.catalog,
            revision: self.revision,
            workspace: &self.workspace,
            documents: &mut self.documents,
            parser: &self.parser,
            cancellation: &self.cancellation,
            used_buffers: &mut self.used,
            heading_id_prefix: &self.prefix,
        }
    }

    fn heading(
        &mut self,
        uri: &str,
        position: Position,
        name: &str,
    ) -> Result<Vec<DocumentEdits>, ResponseError> {
        self.context().heading(uri, position, name)
    }

    fn moves(&mut self, files: &[(&str, &str)]) -> Result<Vec<DocumentEdits>, ResponseError> {
        let files: Vec<_> = files
            .iter()
            .map(|(old, new)| FileRename {
                old_uri: self.directory.uri(old),
                new_uri: self.directory.uri(new),
            })
            .collect();
        self.context().files(&files)
    }

    fn texts(&self, edits: Vec<DocumentEdits>) -> BTreeMap<String, String> {
        edits
            .into_iter()
            .map(|edits| {
                let text = self
                    .documents
                    .get(&edits.uri)
                    .map(|document| document.text().unwrap().to_string())
                    .unwrap_or_else(|| {
                        fs::read_to_string(files::file_uri_path(&edits.uri).unwrap()).unwrap()
                    });
                let mut document = Document::new(1, text).unwrap();
                document
                    .change(
                        2,
                        edits
                            .edits
                            .into_iter()
                            .rev()
                            .map(|edit| ContentChange {
                                range: Some(edit.range),
                                text: edit.new_text,
                            })
                            .collect(),
                    )
                    .unwrap();
                (edits.uri, document.text().unwrap().to_string())
            })
            .collect()
    }
}

fn at(line: u32, character: u32) -> Position {
    Position { line, character }
}

#[test]
fn heading_rename_tracks_duplicate_ids_and_uses_unsaved_source_and_referrers() {
    let mut project = Project::new();
    project.disk("target.md", "# Stale disk title");
    let source = "# Intro\n[one](#intro) [two](#intro-2)\n## Intro\n[three](#other)\n# Other\n";
    let uri = project.open("target.md", source);
    project.disk("docs/closed.md", "😀 [one](../target.md#intro \"Title\")\n![two](../target.md#intro-2)\n\n[three]: ../target.md#other\n");
    project.disk("docs/open.md", "# Stale referrer");
    let open_uri = project.open("docs/open.md", "[two](../target.md#intro-2)");
    let edits = project.heading(&uri, at(0, 3), "Other").unwrap();
    assert_eq!(edits.len(), 3);
    assert!(edits
        .iter()
        .filter(|edits| edits.uri == uri || edits.uri == open_uri)
        .all(|edits| edits.version == Some(1)));
    assert!(edits
        .iter()
        .find(|edits| edits.uri == project.directory.uri("docs/closed.md"))
        .unwrap()
        .version
        .is_none());
    let changed = project.texts(edits);
    assert_eq!(
        changed[&uri],
        "# Other\n[one](#other) [two](#intro)\n## Intro\n[three](#other-2)\n# Other\n"
    );
    assert_eq!(changed[&project.directory.uri("docs/closed.md")], "😀 [one](../target.md#other \"Title\")\n![two](../target.md#intro)\n\n[three]: ../target.md#other-2\n");
    assert_eq!(changed[&open_uri], "[two](../target.md#intro)");
    assert_eq!(
        project.documents[&uri].text().unwrap(),
        source,
        "refactor mutated live text"
    );
    assert_eq!(
        fs::read_to_string(project.directory.0.join("target.md")).unwrap(),
        "# Stale disk title"
    );
}

#[test]
fn literal_heading_names_preserve_atx_setext_and_container_structure() {
    for (source, position, name, expected) in [
        (
            "# A **Old** #\nbody",
            at(0, 4),
            "New [title] *x* &copy;",
            "# New \\[title\\] \\*x\\* \\&copy\\; #\nbody",
        ),
        (
            "Old\nTitle\n-----\nbody",
            at(0, 1),
            "New title",
            "New title\n-----\nbody",
        ),
        (
            "> ## Old\n> Body",
            at(0, 6),
            "New 😀",
            "> ## New 😀\n> Body",
        ),
        ("#\n", at(0, 1), "New", "# New\n"),
        (
            "# Old",
            at(0, 3),
            "http://example.test",
            "# http\\:\\/\\/example\\.test",
        ),
    ] {
        let mut project = Project::new();
        let uri = project.open("target.md", source);
        let edits = project
            .heading(&uri, position, name)
            .unwrap_or_else(|error| panic!("{source}: {}", error.message));
        assert_eq!(project.texts(edits)[&uri], expected, "{source}");
    }
}

#[test]
fn heading_rename_preserves_prefixes_and_decodes_fragments_once() {
    let mut project = Project::new();
    project.prefix = "h-".to_string();
    let uri = project.open("target 中文.md", "# 中文😀");
    project.disk(
        "source.md",
        "[go](target%20%E4%B8%AD%E6%96%87.md#h-%E4%B8%AD%E6%96%87%F0%9F%98%80)",
    );
    let edits = project.heading(&uri, at(0, 3), "新标题 🦀").unwrap();
    let changed = project.texts(edits);
    assert_eq!(
        changed[&project.directory.uri("source.md")],
        "[go](target%20%E4%B8%AD%E6%96%87.md#h-%E6%96%B0%E6%A0%87%E9%A2%98-%F0%9F%A6%80)"
    );
}

#[test]
fn refuses_heading_names_that_capture_missing_anchors_or_remove_addressable_ids() {
    let mut project = Project::new();
    let uri = project.open("target.md", "# Old");
    project.disk("source.md", "[future](target.md#new)");
    let error = project.heading(&uri, at(0, 3), "New").err().unwrap();
    assert!(error.message.contains("activate"));
    assert_eq!(
        project.heading(&uri, at(0, 3), "!!!").err().unwrap().code,
        -32803
    );
    for name in ["", " leading", "trailing ", "line\nbreak"] {
        assert_eq!(
            project.heading(&uri, at(0, 3), name).err().unwrap().code,
            -32602
        );
    }
    assert_eq!(project.documents[&uri].text().unwrap(), "# Old");
}

#[test]
fn anchor_changes_require_roots_for_files_but_allow_untitled_and_local_case_changes() {
    let mut project = Project::new();
    let uri = project.open("target.md", "# Old");
    project.workspace = Workspace::default();
    assert_eq!(
        project.heading(&uri, at(0, 3), "New").err().unwrap().code,
        -32803
    );
    let edits = project.heading(&uri, at(0, 3), "OLD").unwrap();
    assert_eq!(project.texts(edits)[&uri], "# OLD");
    let uri = "untitled:heading";
    project.documents.insert(
        uri.to_string(),
        Document::new(1, "# Old\n[go](#old)".to_string()).unwrap(),
    );
    let edits = project.heading(uri, at(0, 3), "New").unwrap();
    assert_eq!(project.texts(edits)[uri], "# New\n[go](#new)");
}

#[test]
fn file_moves_update_incoming_outgoing_and_explicit_self_links() {
    let mut project = Project::new();
    project.disk("docs/guide.md", "# Disk title");
    project.disk("docs/peer.md", "# Peer");
    project.disk("assets/pic.png", "image fixture");
    project.disk("index.md", "[guide](docs/guide.md?view=raw#intro)");
    let source = "# Intro\n[self](guide.md#intro) [local](#intro)\n[asset](../assets/pic.png \"cover\")\n[peer](peer.md#peer)";
    let uri = project.open("docs/guide.md", source);
    let edits = project
        .moves(&[("docs/guide.md", "manuals/sub/moved 中文.md")])
        .unwrap();
    let changed = project.texts(edits);
    assert_eq!(changed[&uri], "# Intro\n[self](moved%20%E4%B8%AD%E6%96%87.md#intro) [local](#intro)\n[asset](../../assets/pic.png \"cover\")\n[peer](../../docs/peer.md#peer)");
    assert_eq!(
        changed[&project.directory.uri("index.md")],
        "[guide](manuals/sub/moved%20%E4%B8%AD%E6%96%87.md?view=raw#intro)"
    );
    assert_eq!(project.documents[&uri].text().unwrap(), source);
    assert!(project.directory.0.join("docs/guide.md").exists());
    assert!(!project
        .directory
        .0
        .join("manuals/sub/moved 中文.md")
        .exists());
}

#[test]
fn batch_file_swaps_follow_original_targets_and_keep_edit_uris_before_the_move() {
    let mut project = Project::new();
    project.disk("a.md", "# A\n[b](b.md#b)");
    project.disk("b.md", "# B\n[a](a.md#a)");
    project.disk("index.md", "[a](a.md#a) [b](b.md#b)");
    let edits = project
        .moves(&[("a.md", "b.md"), ("b.md", "a.md")])
        .unwrap();
    let changed = project.texts(edits);
    assert_eq!(changed[&project.directory.uri("a.md")], "# A\n[b](a.md#b)");
    assert_eq!(changed[&project.directory.uri("b.md")], "# B\n[a](b.md#a)");
    assert_eq!(
        changed[&project.directory.uri("index.md")],
        "[a](b.md#a) [b](a.md#b)"
    );
}

#[test]
fn asset_moves_preserve_absolute_file_links_in_untitled_buffers() {
    let mut project = Project::new();
    project.disk("old.png", "image fixture");
    let uri = "untitled:scratch";
    project.documents.insert(
        uri.to_string(),
        Document::new(
            1,
            format!("![alt]({}?raw#view)", project.directory.uri("old.png")),
        )
        .unwrap(),
    );
    let edits = project.moves(&[("old.png", "new 名.png")]).unwrap();
    assert_eq!(
        project.texts(edits)[uri],
        format!("![alt]({}?raw#view)", project.directory.uri("new 名.png"))
    );
}

#[test]
fn collisions_out_of_sync_buffers_and_unreadable_markdown_abort_the_whole_rename() {
    let mut project = Project::new();
    project.disk("a.md", "# A");
    project.disk("b.md", "# B");
    assert_eq!(
        project.moves(&[("a.md", "b.md")]).err().unwrap().code,
        -32803
    );
    assert_eq!(
        project
            .moves(&[("a.md", "new.md"), ("b.md", "new.md")])
            .err()
            .unwrap()
            .code,
        -32803
    );
    fs::create_dir(project.directory.0.join("folder")).unwrap();
    assert!(project.moves(&[("folder", "moved")]).unwrap().is_empty());
    let uri = project.open("a.md", "# A");
    let other = project.open("b.md", "[a](a.md#a)");
    project
        .documents
        .get_mut(&other)
        .unwrap()
        .invalidate(2)
        .unwrap();
    assert_eq!(
        project.heading(&uri, at(0, 2), "New").err().unwrap().code,
        -32801
    );
    project.documents.remove(&other);
    fs::write(project.directory.0.join("invalid.md"), [0xff]).unwrap();
    assert_eq!(
        project.heading(&uri, at(0, 2), "New").err().unwrap().code,
        -32803
    );
    assert_eq!(project.documents[&uri].text().unwrap(), "# A");
}

#[test]
fn directory_moves_update_incoming_and_outgoing_links_and_unsaved_descendants() {
    let mut project = Project::new();
    project.disk(
        "old/guide.md",
        "# Guide\n[asset](asset.png) [shared](../shared.md#s)",
    );
    project.disk("old/asset.png", "asset");
    project.disk(
        "old/sub/peer.md",
        "[guide](../guide.md#guide) [outside](../../shared.md)",
    );
    project.disk("shared.md", "# S");
    project.disk(
        "index.md",
        "[guide](old/guide.md?raw#guide) [folder](old/) ![asset](old/asset.png)",
    );
    let draft = project.open(
        "old/draft.md",
        "[shared](../shared.md) [guide](guide.md#guide)",
    );
    let edits = project.moves(&[("old", "archive/deep")]).unwrap();
    assert_eq!(
        edits.iter().find(|edit| edit.uri == draft).unwrap().version,
        Some(1)
    );
    let changed = project.texts(edits);
    assert_eq!(changed[&project.directory.uri("index.md")], "[guide](archive/deep/guide.md?raw#guide) [folder](archive/deep/) ![asset](archive/deep/asset.png)");
    assert_eq!(
        changed[&project.directory.uri("old/guide.md")],
        "# Guide\n[asset](asset.png) [shared](../../shared.md#s)"
    );
    assert_eq!(
        changed[&project.directory.uri("old/sub/peer.md")],
        "[guide](../guide.md#guide) [outside](../../../shared.md)"
    );
    assert_eq!(
        changed[&draft],
        "[shared](../../shared.md) [guide](guide.md#guide)"
    );
    assert!(project.directory.0.join("old/guide.md").is_file());
    assert!(!project.directory.0.join("archive").exists());
}

#[test]
fn directory_swaps_are_simultaneous_and_path_prefixes_respect_component_boundaries() {
    let mut project = Project::new();
    project.disk("a/page.md", "# A\n[self](page.md#a)");
    project.disk("b/page.md", "# B\n[self](page.md#b)");
    project.disk("ab/page.md", "# Unrelated");
    project.disk(
        "index.md",
        "[a](a/page.md#a) [b](b/page.md#b) [ab](ab/page.md)",
    );
    let edits = project.moves(&[("a", "b"), ("b", "a")]).unwrap();
    let changed = project.texts(edits);
    assert_eq!(changed.len(), 1);
    assert_eq!(
        changed[&project.directory.uri("index.md")],
        "[a](b/page.md#a) [b](a/page.md#b) [ab](ab/page.md)"
    );
}

#[test]
fn directory_moves_reject_overlapping_batches_roots_and_existing_destinations() {
    let mut project = Project::new();
    project.disk("a/page.md", "# A");
    project.disk("b/page.md", "# B");
    for moves in [
        vec![("a", "a/sub")],
        vec![("a", "b")],
        vec![("", "moved")],
        vec![("a", "x"), ("a/page.md", "other.md")],
        vec![("a", "x"), ("b", "x/sub")],
        vec![("a", "b/sub"), ("b", "x")],
    ] {
        assert_eq!(
            project.moves(&moves).err().unwrap().code,
            -32803,
            "{moves:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_operations_and_ambiguous_buffer_aliases_are_rejected() {
    let mut project = Project::new();
    project.disk("guide.md", "# Old");
    std::os::unix::fs::symlink("guide.md", project.directory.0.join("alias.md")).unwrap();
    assert_eq!(
        project.moves(&[("alias.md", "new.md")]).err().unwrap().code,
        -32803
    );
    let uri = project.open("guide.md", "# Old");
    project.open("alias.md", "# Different unsaved title");
    assert_eq!(
        project.heading(&uri, at(0, 3), "New").err().unwrap().code,
        -32803
    );
}

#[test]
fn edits_discovered_before_a_disk_change_are_rejected_at_final_validation() {
    let mut project = Project::new();
    let uri = project.open("target.md", "# Old");
    project.disk("a.md", "[go](target.md#old)");
    project.disk("z.md", "[trigger](change)");
    let path = project.directory.0.join("a.md");
    let changed = Arc::new(AtomicBool::new(false));
    let once = Arc::clone(&changed);
    project.parser = YozoraParser::new(DefaultParserProps {
        default_parse_options: Some(ParseOptions {
            format_url: Some(Arc::new(move |url| {
                if url == "change" && !once.swap(true, Ordering::SeqCst) {
                    fs::write(&path, "external edit").unwrap();
                }
                url.to_string()
            })),
            ..ParseOptions::default()
        }),
        ..DefaultParserProps::default()
    });
    assert_eq!(
        project.heading(&uri, at(0, 3), "New").err().unwrap().code,
        -32801
    );
    assert!(changed.load(Ordering::SeqCst));
}

#[test]
fn cancellation_before_analysis_does_not_change_sources() {
    let mut project = Project::new();
    let uri = project.open("target.md", "# Old");
    project.cancellation.cancel();
    assert_eq!(
        project.heading(&uri, at(0, 3), "New").err().unwrap().code,
        -32800
    );
    assert_eq!(project.documents[&uri].text().unwrap(), "# Old");
}

#[test]
fn cached_refactors_accept_different_names_and_recheck_closed_source_text() {
    let mut project = Project::new();
    project.revision = Some(1);
    let uri = project.open("target.md", "# Old\n\n## Other");
    project.disk("a.md", "[go](target.md#old) [other](target.md#other)");
    for name in ["First", "Second", "中文", "Fourth"] {
        let edits = project.heading(&uri, at(0, 3), name).unwrap();
        let texts = project.texts(edits);
        assert_eq!(texts[&uri], format!("# {name}\n\n## Other"));
        assert!(texts[&project.directory.uri("a.md")].contains(&format!(
            "target.md#{}",
            files::encode_component(&name.to_lowercase())
        )));
        assert!(texts[&project.directory.uri("a.md")].contains("target.md#other"));
    }
    assert_eq!(project.index.entries.len(), 1);

    // Even an edit without a delivered file event must never yield a stale
    // WorkspaceEdit. A later event then refreshes the cached source summary.
    project.disk("a.md", "[changed](target.md#old) [other](target.md#other)");
    assert_eq!(
        project.heading(&uri, at(0, 3), "Fifth").err().unwrap().code,
        -32801
    );
    project.revision = Some(2);
    let edits = project.heading(&uri, at(0, 3), "Fifth").unwrap();
    let texts = project.texts(edits);
    assert_eq!(
        texts[&project.directory.uri("a.md")],
        "[changed](target.md#fifth) [other](target.md#other)"
    );

    let edits = project.heading(&uri, at(2, 4), "Another").unwrap();
    let texts = project.texts(edits);
    assert_eq!(
        texts[&project.directory.uri("a.md")],
        "[changed](target.md#old) [other](target.md#another)"
    );
}

#[test]
fn event_refactors_refresh_changed_sources_before_producing_new_edits() {
    let mut project = Project::new();
    project.revision = Some(1);
    project.validate_disk = false;
    Arc::make_mut(&mut project.catalog.events).record(1, None);
    let uri = project.open("target.md", "# Old");
    project.disk("source.md", "[go](target.md#old)");
    for (revision, source, name) in [
        (1, "[go](target.md#old)", "First"),
        (2, "\n\n[changed](target.md#old)", "Second"),
        (3, "[unrelated](target.md#missing)", "Third"),
    ] {
        if revision != 1 {
            project.disk("source.md", source);
            project.revision = Some(revision);
            Arc::make_mut(&mut project.catalog.events).record(
                revision,
                Some(&[crate::protocol::FileEvent {
                    uri: project.directory.uri("source.md"),
                    kind: 2,
                }]),
            );
        }
        let edits = project.heading(&uri, at(0, 3), name).unwrap();
        let texts = project.texts(edits);
        assert_eq!(texts[&uri], format!("# {name}"));
        if revision == 3 {
            assert_eq!(texts.len(), 1);
        } else {
            assert_eq!(
                texts[&project.directory.uri("source.md")],
                source.replace("#old", &format!("#{}", name.to_lowercase()))
            );
        }
    }
}

#[test]
fn large_final_validation_checks_all_partitions_and_cancellation() {
    let project = Project::new();
    let expected = Arc::new("# 原文 😀\n".to_string());
    let snapshots: Vec<_> = (0..132)
        .map(|index| {
            let name = format!("source-{index:03}.md");
            project.disk(&name, &expected);
            (project.directory.0.join(name), Arc::clone(&expected))
        })
        .collect();
    let scope = project
        .workspace
        .index_scope(&project.cancellation, files::MAX_WORKSPACE_ROOTS)
        .unwrap();
    validate_sources(&snapshots, &scope, &project.cancellation).unwrap();
    for index in [0, 33, 66, 99, 131] {
        fs::write(&snapshots[index].0, "externally modified").unwrap();
        assert_eq!(
            validate_sources(&snapshots, &scope, &project.cancellation)
                .unwrap_err()
                .code,
            -32801
        );
        fs::write(&snapshots[index].0, expected.as_str()).unwrap();
    }
    project.cancellation.cancel();
    assert_eq!(
        validate_sources(&snapshots, &scope, &project.cancellation)
            .unwrap_err()
            .code,
        -32800
    );
}

#[test]
fn deeply_nested_referrers_are_rewritten_and_validated_iteratively() {
    let mut project = Project::new();
    let uri = "untitled:deep";
    let source = format!(
        "# Old\n\n{}[go](#old){}",
        "**".repeat(4_000),
        "**".repeat(4_000)
    );
    project
        .documents
        .insert(uri.to_string(), Document::new(1, source).unwrap());
    let edits = project.heading(uri, at(0, 3), "New").unwrap();
    let changed = project.texts(edits);
    assert!(changed[uri].starts_with("# New\n"));
    assert!(changed[uri].contains("[go](#new)"));
}
