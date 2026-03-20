use std::path::PathBuf;

use yozora_suitecases::{format_unexpected_report, run_fixture_subset, SuiteRunOptions};

mod support;
use support::{env_assert_level, ParserSuiteAdapter};

#[test]
fn fixture_gfm_subset_l1() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_root = repo_root.join("fixtures");
    let fixture_paths = [
        "gfm/heading/#032.json",
        "gfm/inline-code/#338.json",
        "gfm/emphasis/rule#1/#360.json",
        "gfm/thematic-break/#013.json",
        "gfm/list/#281.json",
        "gfm/setext-heading/#050.json",
        "gfm/blockquote/#206.json",
        "gfm/indented-code/#077.json",
        "gfm/fenced-code/#089.json",
        "gfm/link/#493.json",
        "gfm/image/#580.json",
        "gfm/autolink/#602.json",
        "gfm/html-inline/#632.json",
        "gfm/break/hard line breaks/#654.json",
        "gfm/definition/#161.json",
        "gfm/link-reference/collapsed/#561.json",
        "gfm/image-reference/collapsed/#592.json",
        "gfm/html-block/#119.json",
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
