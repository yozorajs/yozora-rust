use std::path::PathBuf;

use yozora_suitecases::{format_unexpected_report, run_fixture_subset, SuiteRunOptions};

mod support;
use support::{env_assert_level, ParserSuiteAdapter};

#[test]
fn fixture_gfm_subset_l1() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_root = repo_root.join("fixtures");
    let fixture_paths = [
        "gfm/#013.json",
        "gfm/#032.json",
        "gfm/#050.json",
        "gfm/#077.json",
        "gfm/#089.json",
        "gfm/#118.json",
        "gfm/#161.json",
        "gfm/#206.json",
        "gfm/#281.json",
        "gfm/#338.json",
        "gfm/#360.json",
        "gfm/#494.json",
        "gfm/#562.json",
        "gfm/#581.json",
        "gfm/#593.json",
        "gfm/#603.json",
        "gfm/#636.json",
        "gfm/#658.json",
    ];

    let assert_level = env_assert_level();
    let adapter = ParserSuiteAdapter::new("gfm", assert_level);
    let options = SuiteRunOptions {
        fixtures_root: &fixtures_root,
        parser_profile: "gfm",
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
