use std::path::PathBuf;

use yozora_suitecases::{
    format_unexpected_report, is_fixture_enabled_for_profile, parse_known_failures_toml,
    parse_profile_map_toml, run_fixture_subset, AssertLevel, SuiteAdapter, SuiteRunOptions,
};

mod support;
use support::{env_assert_level, env_parser_profile, ParserSuiteAdapter};

#[test]
fn suitecases_smoke_case() {
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

    let parser_profile = env_parser_profile();
    let assert_level = env_assert_level();

    let fixture_rel = "gfm/#032.json";
    let fixtures_root = repo_root.join("fixtures");
    let fixture_path = fixtures_root.join(fixture_rel);
    let enabled = is_fixture_enabled_for_profile(&profile_map, &parser_profile, fixture_rel)
        .expect("parser profile should exist in profile-map.toml");
    assert!(
        enabled,
        "fixture should be enabled for selected parser profile"
    );

    assert!(fixture_path.exists(), "smoke fixture should exist");

    let adapter = ParserSuiteAdapter::new(&parser_profile, assert_level);
    let options = SuiteRunOptions {
        fixtures_root: &fixtures_root,
        parser_profile: &parser_profile,
        assert_level,
        profile_map: Some(&profile_map),
        known_failures: &known_failures,
    };

    let report = run_fixture_subset(&adapter, &options, &[fixture_rel]);
    assert!(report.total_cases > 0, "smoke fixture should contain cases");
    assert!(
        report.is_clean(),
        "{}",
        format_unexpected_report(&report, 20)
    );
}

#[test]
fn backtick_required_profile_disables_optional_inline_math() {
    let adapter = ParserSuiteAdapter::new("yozora_inline_math_backtick_required", AssertLevel::L1);

    let plain = adapter.run_case("$x$").expect("parse plain math syntax");
    assert_eq!(plain["children"][0]["type"], "paragraph");
    assert_eq!(plain["children"][0]["children"][0]["type"], "text");
    assert_eq!(plain["children"][0]["children"][0]["value"], "$x$");

    let wrapped = adapter
        .run_case("`$x$`")
        .expect("parse backtick-wrapped math syntax");
    assert_eq!(wrapped["children"][0]["children"][0]["type"], "inlineMath");
    assert_eq!(wrapped["children"][0]["children"][0]["value"], "x");
}
