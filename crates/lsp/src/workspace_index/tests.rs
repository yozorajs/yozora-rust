use std::fs::{self, File};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use yozora_core_parser::DefaultParserProps;

use super::*;
use crate::document::MAX_DOCUMENT_BYTES;
use crate::files::tests::TestDir;
use crate::protocol::Position;

struct TestIndex {
    index: Index,
    workspace: Workspace,
    documents: HashMap<String, Document>,
    parser: YozoraParser,
    used: HashSet<String>,
}

impl TestIndex {
    fn new(directory: &TestDir) -> Self {
        Self {
            index: Index::default(),
            workspace: Workspace::new(vec![directory.uri("")]),
            documents: HashMap::new(),
            parser: YozoraParser::default(),
            used: HashSet::new(),
        }
    }

    fn symbols(&mut self, query: &str) -> Result<Vec<SymbolInformation>, ResponseError> {
        self.used.clear();
        self.index.symbols(
            query,
            &self.workspace,
            &mut self.documents,
            &self.parser,
            &Cancellation::default(),
            &mut self.used,
        )
    }

    fn names(&mut self, query: &str) -> Vec<String> {
        self.symbols(query)
            .unwrap()
            .into_iter()
            .map(|symbol| symbol.name)
            .collect()
    }

    fn open(&mut self, uri: String, text: &str) {
        self.documents
            .insert(uri, Document::new(1, text.to_string()).unwrap());
    }
}

#[test]
fn finds_unopened_unicode_and_nested_headings_with_outline_locations() {
    let directory = TestDir::new();
    fs::create_dir(directory.0.join("docs")).unwrap();
    fs::write(directory.0.join("a.yozora"), "# First").unwrap();
    fs::write(directory.0.join("ignored.txt"), "# Not Markdown").unwrap();
    fs::write(
        directory.0.join("docs/指南 %.MD"),
        "# Parent 😀\r\n\r\n> ## Äpfel **中文😀**\r\n\r\nSetext 文\r\n---\r\n",
    )
    .unwrap();
    let mut test = TestIndex::new(&directory);
    assert_eq!(
        test.names("  "),
        ["First", "Parent 😀", "Äpfel 中文😀", "Setext 文"]
    );
    let symbols = test.symbols(" äPFEL ").unwrap();
    assert_eq!(symbols.len(), 1);
    let symbol = &symbols[0];
    assert_eq!(symbol.name, "Äpfel 中文😀");
    assert_eq!(symbol.kind, 15);
    assert_eq!(symbol.container_name.as_deref(), Some("Parent 😀"));
    assert_eq!(symbol.location.uri, directory.uri("docs/指南 %.MD"));
    assert_eq!(
        symbol.location.range,
        Range {
            start: Position {
                line: 2,
                character: 5
            },
            end: Position {
                line: 2,
                character: 19
            },
        }
    );
    assert_eq!(test.names("中文😀"), ["Äpfel 中文😀"]);
    assert!(test.names("not present").is_empty());
    assert!(test.used.is_empty());
}

#[test]
fn content_cache_observes_same_length_edits_creates_deletes_and_invalid_files() {
    let directory = TestDir::new();
    let path = directory.0.join("guide.md");
    fs::write(&path, "# One [link](count)").unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let mut test = TestIndex::new(&directory);
    test.parser = YozoraParser::new(DefaultParserProps {
        default_parse_options: Some(ParseOptions {
            format_url: Some(Arc::new(move |url| {
                count.fetch_add(1, Ordering::SeqCst);
                url.to_string()
            })),
            ..ParseOptions::default()
        }),
        ..DefaultParserProps::default()
    });
    assert_eq!(test.names(""), ["One link"]);
    assert_eq!(test.names("LINK"), ["One link"]);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "unchanged content was reparsed"
    );
    // Equal lengths must not hide an edit behind metadata-based cache keys.
    fs::write(&path, "# Two [link](count)").unwrap();
    assert_eq!(test.names(""), ["Two link"]);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    fs::remove_file(&path).unwrap();
    assert!(test.names("").is_empty());
    assert!(test.index.cache.is_empty());
    fs::write(&path, "# New [link](count)").unwrap();
    assert_eq!(test.names(""), ["New link"]);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    fs::write(&path, [0xff]).unwrap();
    assert!(test.names("").is_empty());
    fs::write(&path, "# Small").unwrap();
    assert_eq!(test.names(""), ["Small"]);
    File::create(&path)
        .unwrap()
        .set_len((MAX_DOCUMENT_BYTES + 1) as u64)
        .unwrap();
    assert!(test.names("").is_empty());
    assert!(test.index.cache.is_empty());
}

#[test]
fn unsaved_buffers_override_disk_and_out_of_sync_text_never_falls_back() {
    let directory = TestDir::new();
    let uri = directory.uri("guide.md");
    fs::write(directory.0.join("guide.md"), "# Disk").unwrap();
    let mut test = TestIndex::new(&directory);
    assert_eq!(test.names(""), ["Disk"]);
    test.open(uri.clone(), "# Unsaved");
    test.open(directory.uri("missing/new.md"), "# New");
    test.open("untitled:scratch".to_string(), "# Scratch");
    assert_eq!(test.names(""), ["Unsaved", "New", "Scratch"]);
    assert_eq!(test.used.len(), 3);
    assert!(
        test.index.cache.is_empty(),
        "overlaid disk cache was retained"
    );
    test.documents.get_mut(&uri).unwrap().invalidate(2).unwrap();
    assert_eq!(test.symbols("Disk").unwrap_err().code, -32801);
    test.documents.remove(&uri);
    assert_eq!(test.names(""), ["Disk", "New", "Scratch"]);
    test.open(uri, "# Reopened");
    assert_eq!(test.names(""), ["Reopened", "New", "Scratch"]);
}

#[test]
fn absent_and_remote_roots_only_search_open_buffers_and_root_changes_prune_cache() {
    let directory = TestDir::new();
    let other = TestDir::new();
    fs::write(directory.0.join("disk.md"), "# Disk").unwrap();
    fs::write(other.0.join("other.md"), "# Other").unwrap();
    let mut test = TestIndex::new(&directory);
    test.open(directory.uri("open.md"), "# Open");
    test.open(
        "vscode-remote://host/docs/remote.md".to_string(),
        "# Remote buffer",
    );
    assert_eq!(test.names(""), ["Disk", "Open", "Remote buffer"]);
    test.workspace
        .change(&[directory.uri("")], vec![other.uri("")]);
    let symbols = test.symbols("").unwrap();
    assert_eq!(symbols.len(), 3);
    assert!(symbols.iter().any(|symbol| symbol.name == "Other"));
    assert!(!symbols.iter().any(|symbol| symbol.name == "Disk"));
    assert_eq!(
        test.index.cache.keys().collect::<Vec<_>>(),
        [&other.0.join("other.md")]
    );
    for folders in [vec![], vec!["vscode-remote://host/docs".to_string()]] {
        test.workspace = Workspace::new(folders);
        assert_eq!(test.names(""), ["Open", "Remote buffer"]);
        assert!(test.index.cache.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn aliases_select_canonical_then_lexical_then_other_open_uris_once() {
    let directory = TestDir::new();
    fs::write(directory.0.join("guide.md"), "# Disk").unwrap();
    std::os::unix::fs::symlink("guide.md", directory.0.join("alias.md")).unwrap();
    let canonical = directory.uri("guide.md");
    let lexical = canonical.replace("guide.md", "%67uide.md");
    let alias = directory
        .uri("alias.md")
        .replacen("file://", "FILE://localhost", 1);
    let mut test = TestIndex::new(&directory);
    assert_eq!(test.names(""), ["Disk"]);
    test.open(alias.clone(), "# Alias");
    test.open(lexical.clone(), "# Lexical");
    test.open(canonical.clone(), "# Canonical");
    for (uri, name) in [
        (canonical, "Canonical"),
        (lexical, "Lexical"),
        (alias, "Alias"),
    ] {
        let symbols = test.symbols("").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, name);
        assert_eq!(symbols[0].location.uri, uri);
        assert_eq!(test.used, HashSet::from([uri.clone()]));
        test.documents.remove(&uri);
    }
    assert_eq!(test.names(""), ["Disk"]);
}

#[test]
fn query_limits_return_errors_and_a_narrower_query_recovers_from_response_limits() {
    let directory = TestDir::new();
    fs::write(directory.0.join("guide.md"), "# First\n\n## Second").unwrap();
    for limits in [
        Limits {
            roots: 0,
            ..Limits::default()
        },
        Limits {
            entries: 0,
            ..Limits::default()
        },
        Limits {
            files: 0,
            ..Limits::default()
        },
        Limits {
            text_bytes: 4,
            ..Limits::default()
        },
        Limits {
            summary_bytes: 4,
            ..Limits::default()
        },
        Limits {
            result_bytes: 4,
            ..Limits::default()
        },
        Limits {
            results: 1,
            ..Limits::default()
        },
    ] {
        let mut test = TestIndex::new(&directory);
        test.index.limits = limits;
        assert_eq!(test.symbols("").unwrap_err().code, -32000);
        if limits.results == 1 {
            assert_eq!(test.names("Second"), ["Second"]);
        }
    }
    let mut test = TestIndex::new(&directory);
    test.index.limits.files = 1;
    test.open(directory.uri("unsaved.md"), "# Unsaved");
    assert_eq!(test.symbols("").unwrap_err().code, -32000);
    test.index.limits.files = 2;
    assert_eq!(test.names(""), ["First", "Second", "Unsaved"]);
}

#[test]
fn cache_eviction_never_changes_results_or_exceeds_its_payload_budget() {
    let directory = TestDir::new();
    let mut test = TestIndex::new(&directory);
    test.index.limits.cache_bytes = 2_000;
    for (a, b) in [(100, 1_000), (1_000, 100), (100, 1_000)] {
        fs::write(directory.0.join("a.md"), format!("# A\n{}", "a".repeat(a))).unwrap();
        fs::write(directory.0.join("b.md"), format!("# B\n{}", "b".repeat(b))).unwrap();
        assert_eq!(test.names(""), ["A", "B"]);
        assert!(test.index.cache_bytes <= test.index.limits.cache_bytes);
    }
    // Equal paths can have different allocation capacities across discoveries.
    // Removing an entry must release the budget charged for its owned key.
    test.index.limits.cache_bytes = 4_096;
    let (path, entry) = test.index.cache.pop_first().unwrap();
    let mut padded = PathBuf::with_capacity(path.capacity() + 1_024);
    padded.push(path);
    test.index.cache.insert(padded, entry);
    test.index.cache_bytes = test
        .index
        .cache
        .iter()
        .map(|(path, entry)| entry.owned_bytes(path))
        .sum();
    assert_eq!(test.names(""), ["A", "B"]);
    assert_eq!(
        test.index.cache_bytes,
        test.index
            .cache
            .iter()
            .map(|(path, entry)| entry.owned_bytes(path))
            .sum::<usize>()
    );
    test.index.limits.cache_bytes = 1;
    // Lowering this internal test limit also starts with a cold cache.
    test.index.cache.clear();
    test.index.cache_bytes = 0;
    assert_eq!(test.names(""), ["A", "B"]);
    assert!(test.index.cache.is_empty());
}

#[test]
fn skipped_non_utf8_files_still_consume_the_read_budget() {
    let directory = TestDir::new();
    fs::write(directory.0.join("a.md"), [0xff; 8]).unwrap();
    fs::write(directory.0.join("b.md"), "# Valid").unwrap();
    let mut test = TestIndex::new(&directory);
    test.index.limits.text_bytes = 10;
    assert_eq!(test.symbols("").unwrap_err().code, -32000);
    test.index.limits.text_bytes = 15;
    assert_eq!(test.names(""), ["Valid"]);
}

#[test]
fn repeated_parent_names_have_linear_cache_space_and_bounded_response_space() {
    let directory = TestDir::new();
    fs::write(
        directory.0.join("guide.md"),
        format!("# {}\n{}", "p".repeat(16_000), "## Child\n".repeat(1_000)),
    )
    .unwrap();
    let mut test = TestIndex::new(&directory);
    test.index.limits.summary_bytes = 128 * 1024;
    assert!(test.names("absent").is_empty());
    assert!(test.index.cache[&directory.0.join("guide.md")].summary_bytes < 128 * 1024);
    assert_eq!(test.symbols("Child").unwrap_err().code, -32000);
}

#[test]
fn cancellation_before_and_during_disk_parsing_prevents_later_file_analysis() {
    let directory = TestDir::new();
    fs::write(directory.0.join("a.md"), "# First [link](stop)").unwrap();
    fs::write(directory.0.join("b.md"), "# Second [link](unexpected)").unwrap();
    let token = Cancellation::default();
    let cancel = token.clone();
    let parser = YozoraParser::new(DefaultParserProps {
        default_parse_options: Some(ParseOptions {
            format_url: Some(Arc::new(move |url| {
                assert_eq!(url, "stop", "analysis continued after cancellation");
                cancel.cancel();
                url.to_string()
            })),
            ..ParseOptions::default()
        }),
        ..DefaultParserProps::default()
    });
    let mut test = TestIndex::new(&directory);
    for _ in 0..2 {
        let result = test.index.symbols(
            "",
            &test.workspace,
            &mut test.documents,
            &parser,
            &token,
            &mut test.used,
        );
        assert_eq!(result.unwrap_err().code, -32800);
    }
    assert_eq!(test.names(""), ["First link", "Second link"]);
}

#[cfg(unix)]
#[test]
fn rechecks_paths_replaced_with_symlinks_between_discovery_and_reading() {
    let directory = TestDir::new();
    let outside = TestDir::new();
    let target = directory.0.join("b.md");
    let escaped = outside.0.join("outside.md");
    fs::write(directory.0.join("a.md"), "# First [link](replace)").unwrap();
    fs::write(&target, "# Old target").unwrap();
    fs::write(&escaped, "# Outside [link](unexpected)").unwrap();
    let mut test = TestIndex::new(&directory);
    test.parser = YozoraParser::new(DefaultParserProps {
        default_parse_options: Some(ParseOptions {
            format_url: Some(Arc::new(move |url| {
                assert_eq!(url, "replace", "a replaced path escaped the workspace");
                fs::remove_file(&target).unwrap();
                std::os::unix::fs::symlink(&escaped, &target).unwrap();
                url.to_string()
            })),
            ..ParseOptions::default()
        }),
        ..DefaultParserProps::default()
    });
    assert_eq!(test.names(""), ["First link"]);
    assert_eq!(test.names(""), ["First link"]);
}
