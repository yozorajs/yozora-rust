use std::path::PathBuf;

use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;
use yozora_test_util::{
    compare_parse_answer, expand_fixture_cases, is_fixture_enabled_for_profile, is_known_failure,
    load_fixture_document, parse_known_failures_toml, parse_profile_map_toml, AssertLevel,
};

#[test]
fn fixture_runner_smoke_case() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile_map_path = repo_root.join("fixtures/profile-map.toml");
    let known_failures_path = repo_root.join("fixtures/known-failures.toml");

    let profile_map_text = std::fs::read_to_string(&profile_map_path)
        .expect("failed to read fixtures/profile-map.toml");
    let known_failures_text = std::fs::read_to_string(&known_failures_path)
        .expect("failed to read fixtures/known-failures.toml");

    let profile_map = parse_profile_map_toml(&profile_map_text)
        .expect("failed to parse fixtures/profile-map.toml");
    let known_failures = parse_known_failures_toml(&known_failures_text)
        .expect("failed to parse fixtures/known-failures.toml");

    let parser_profile =
        std::env::var("YOZORA_PARSER_PROFILE").unwrap_or_else(|_| "yozora".to_string());
    let assert_level = std::env::var("YOZORA_ASSERT_LEVEL")
        .ok()
        .and_then(|value| AssertLevel::from_str(&value))
        .unwrap_or(AssertLevel::L1);

    let fixture_rel = "local/smoke/minimal.json";
    let fixture_path = repo_root.join("fixtures").join(fixture_rel);
    let enabled = is_fixture_enabled_for_profile(&profile_map, &parser_profile, fixture_rel)
        .expect("parser profile should exist in profile-map.toml");
    assert!(
        enabled,
        "fixture should be enabled for selected parser profile"
    );

    let doc = load_fixture_document(&fixture_path).expect("failed to load local smoke fixture");
    let cases = expand_fixture_cases(fixture_rel, doc);
    assert!(
        !cases.is_empty(),
        "smoke fixture should contain at least one case"
    );

    let parse_options = Some(ParseOptions {
        should_reserve_position: matches!(assert_level, AssertLevel::L2),
        ..ParseOptions::default()
    });

    for case in &cases {
        let root = match parser_profile.as_str() {
            "yozora" => YozoraParser::default().parse(&case.input, parse_options.clone()),
            "gfm" => GfmParser::default().parse(&case.input, parse_options.clone()),
            "gfm_ex" => GfmExParser::default().parse(&case.input, parse_options.clone()),
            other => panic!("unsupported parser profile: {other}"),
        };

        let actual = serde_json::to_value(root).expect("failed to serialize parse result");
        let expected = case
            .parse_answer
            .as_ref()
            .expect("smoke fixture case should contain parseAnswer");

        if let Err(diff) = compare_parse_answer(expected, &actual, assert_level) {
            let known = is_known_failure(
                &known_failures,
                &case.case_id,
                &parser_profile,
                assert_level,
            );
            assert!(
                known,
                "unexpected fixture diff\nfixture_path={}\ncase_id={}\nparser_profile={}\nassert_level={:?}\n{}",
                case.fixture_path,
                case.case_id,
                parser_profile,
                assert_level,
                diff,
            );
        }
    }
}
