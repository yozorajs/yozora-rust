use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use yozora_core_parser::ParseOptions;
use yozora_markup_weaver::{DefaultMarkupWeaver, MarkupWeaverContract};
use yozora_parser::YozoraParser;

#[test]
fn markup_answers_match_v2_4_0() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root.join("fixtures");
    let excluded_gfm = HashSet::from([
        "#036", "#310", "#333", "#334", "#335", "#336", "#337", "#359", "#503", "#535", "#554",
        "#572", "#601", "#602", "#615", "#625", "#626", "#629", "#632",
    ]);
    let excluded_custom = HashSet::from([
        "custom/math/multiple-line/#2.json",
        "custom/footnote/escape.json",
        "custom/inline-math/backtick-optional/#008.json",
        "custom/inline-math/backtick-required/#008.json",
    ]);

    let mut files = collect_json_files(&fixtures.join("gfm"));
    files.extend(collect_json_files(&fixtures.join("custom")));
    files.sort();

    let parser = YozoraParser::default();
    let weaver = DefaultMarkupWeaver::default();
    let mut failures = Vec::new();

    for file in files {
        let relative = file
            .strip_prefix(&fixtures)
            .expect("fixture should be below fixture root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with("gfm/") {
            let stem = file
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if !stem.starts_with('#') || excluded_gfm.contains(stem) {
                continue;
            }
        } else if excluded_custom.contains(relative.as_str()) {
            continue;
        }

        let document: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let Some(cases) = document.get("cases").and_then(Value::as_array) else {
            continue;
        };
        for (index, case) in cases.iter().enumerate() {
            let Some(expected) = case.get("markupAnswer").and_then(Value::as_str) else {
                continue;
            };
            let input = case
                .get("input")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let ast = parser.parse(
                input,
                Some(ParseOptions {
                    should_reserve_position: Some(true),
                    ..ParseOptions::default()
                }),
            );
            let actual = weaver.weave(&ast);
            if actual != expected {
                failures.push(format!(
                    "{relative}::{index}\nexpected={expected:?}\nactual={actual:?}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} markup differences\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn collect_json_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
                files.push(path);
            }
        }
    }
    files
}
