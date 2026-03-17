use std::path::{Path, PathBuf};

use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;
use yozora_test_util::{
    collect_fixture_files, compare_parse_answer, expand_fixture_cases,
    is_fixture_enabled_for_profile, is_known_failure, load_fixture_document,
    parse_known_failures_toml, parse_profile_map_toml, AssertLevel,
};

#[test]
#[ignore = "批量 fixture 回归，按需手动执行"]
fn fixture_batch_report() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile_map_text = std::fs::read_to_string(repo_root.join("fixtures/profile-map.toml"))
        .expect("read profile-map.toml");
    let known_failures_text =
        std::fs::read_to_string(repo_root.join("fixtures/known-failures.toml"))
            .expect("read known-failures.toml");

    let profile_map = parse_profile_map_toml(&profile_map_text).expect("parse profile-map.toml");
    let known_failures =
        parse_known_failures_toml(&known_failures_text).expect("parse known-failures.toml");

    let parser_profile =
        std::env::var("YOZORA_PARSER_PROFILE").unwrap_or_else(|_| "yozora".to_string());
    let assert_level = std::env::var("YOZORA_ASSERT_LEVEL")
        .ok()
        .and_then(|v| AssertLevel::from_str(&v))
        .unwrap_or(AssertLevel::L1);
    let fail_on_diff = std::env::var("YOZORA_FAIL_ON_DIFF").is_ok_and(|v| v == "1");
    let max_print = std::env::var("YOZORA_REPORT_MAX_PRINT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(20);

    let fixtures_root = repo_root.join("fixtures");
    let fixture_files = collect_fixture_files(&fixtures_root).expect("collect fixture files");

    let parse_options = Some(ParseOptions {
        should_reserve_position: matches!(assert_level, AssertLevel::L2),
    });

    let mut total_cases = 0usize;
    let mut known_failure_cases = 0usize;
    let mut unexpected = Vec::new();

    for fixture_abs in &fixture_files {
        let fixture_rel = rel_path(&fixtures_root, fixture_abs);
        let enabled = is_fixture_enabled_for_profile(&profile_map, &parser_profile, &fixture_rel)
            .unwrap_or(false);
        if !enabled {
            continue;
        }

        let doc = match load_fixture_document(fixture_abs) {
            Ok(doc) => doc,
            Err(err) => {
                unexpected.push(format!("[load] fixture={fixture_rel} error={err}"));
                continue;
            }
        };

        let cases = expand_fixture_cases(&fixture_rel, doc);
        for case in &cases {
            total_cases += 1;
            let root = match parser_profile.as_str() {
                "yozora" => YozoraParser::default().parse(&case.input, parse_options),
                "gfm" => GfmParser::default().parse(&case.input, parse_options),
                "gfm_ex" => GfmExParser::default().parse(&case.input, parse_options),
                other => panic!("unsupported parser profile: {other}"),
            };

            let actual = serde_json::to_value(root).expect("serialize parse result");
            let Some(expected) = case.parse_answer.as_ref() else {
                continue;
            };

            if let Err(diff) = compare_parse_answer(expected, &actual, assert_level) {
                if is_known_failure(
                    &known_failures,
                    &case.case_id,
                    &parser_profile,
                    assert_level,
                ) {
                    known_failure_cases += 1;
                    continue;
                }

                unexpected.push(format!(
                    "fixture={} case_id={} profile={} level={:?} {}",
                    case.fixture_path, case.case_id, parser_profile, assert_level, diff
                ));
            }
        }
    }

    if !unexpected.is_empty() {
        eprintln!(
            "fixture batch report: total_cases={} known_failures={} unexpected={} (showing up to {})",
            total_cases,
            known_failure_cases,
            unexpected.len(),
            max_print
        );
        for item in unexpected.iter().take(max_print) {
            eprintln!("{item}");
        }
    }

    if fail_on_diff {
        assert!(
            unexpected.is_empty(),
            "unexpected fixture diffs found: {}",
            unexpected.len()
        );
    } else {
        assert!(
            total_cases > 0,
            "batch runner should execute at least one case"
        );
    }
}

fn rel_path(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .expect("fixture should be inside root")
        .to_string_lossy()
        .replace('\\', "/")
}
