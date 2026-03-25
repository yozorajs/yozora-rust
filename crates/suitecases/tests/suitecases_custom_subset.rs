use std::path::PathBuf;

use yozora_suitecases::{format_unexpected_report, run_fixture_subset, SuiteRunOptions};

mod support;
use support::{env_assert_level, ParserSuiteAdapter};

#[test]
fn fixture_custom_subset_l1() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_root = repo_root.join("fixtures");
    let fixture_paths = [
        "custom/admonition/basic.json",
        "custom/ecma-import/condition1.json",
        "custom/inline-math/backtick-optional/#001.json",
        "custom/large/unicode_1k.json",
        "custom/math/single-line/#1.json",
        "custom/footnote/basic.json",
        "custom/footnote-definition/basic2.json",
        "custom/table/backslash.json",
    ];

    let assert_level = env_assert_level();
    let adapter = ParserSuiteAdapter::new("yozora", assert_level);
    let options = SuiteRunOptions {
        fixtures_root: &fixtures_root,
        parser_profile: "yozora",
        assert_level,
        profile_map: None,
        known_failures: &[],
    };

    let report = run_fixture_subset(&adapter, &options, &fixture_paths);
    assert!(
        report.is_clean(),
        "{}",
        format_unexpected_report(&report, 20)
    );
    assert!(
        report.total_cases > 0,
        "fixture subset should execute cases"
    );
}
