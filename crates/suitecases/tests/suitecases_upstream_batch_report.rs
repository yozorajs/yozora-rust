use std::path::PathBuf;

use yozora_suitecases::{
    format_unexpected_report, parse_known_failures_toml, parse_profile_map_toml, run_fixture_batch,
    SuiteRunOptions,
};

mod support;
use support::{
    env_assert_level, env_fail_on_diff, env_parser_profile, env_report_max_print,
    ParserSuiteAdapter,
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

    let parser_profile = env_parser_profile();
    let assert_level = env_assert_level();
    let fail_on_diff = env_fail_on_diff();
    let max_print = env_report_max_print();

    let fixtures_root = repo_root.join("fixtures");
    let adapter = ParserSuiteAdapter::new(&parser_profile, assert_level);
    let options = SuiteRunOptions {
        fixtures_root: &fixtures_root,
        parser_profile: &parser_profile,
        assert_level,
        profile_map: Some(&profile_map),
        known_failures: &known_failures,
    };
    let report = run_fixture_batch(&adapter, &options).expect("run fixture batch");

    if !report.unexpected.is_empty() {
        eprintln!("{}", format_unexpected_report(&report, max_print));
    }

    if fail_on_diff {
        assert!(
            report.is_clean(),
            "unexpected fixture diffs found: {}",
            report.unexpected.len()
        );
    } else {
        assert!(
            report.total_cases > 0,
            "batch runner should execute at least one case"
        );
    }
}
