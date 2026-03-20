use std::path::PathBuf;

use yozora_core_parser::ParseOptions;
use yozora_parser_gfm::GfmParser;
use yozora_suitecases::{
    compare_parse_answer, expand_fixture_cases, load_fixture_document, AssertLevel,
};

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

    let assert_level = std::env::var("YOZORA_ASSERT_LEVEL")
        .ok()
        .and_then(|value| AssertLevel::from_str(&value))
        .unwrap_or(AssertLevel::L2);
    let parse_options = Some(ParseOptions {
        shouldReservePosition: Some(matches!(assert_level, AssertLevel::L2)),
        ..ParseOptions::default()
    });

    let parser = GfmParser::default();
    for fixture_rel in fixture_paths {
        let fixture_abs = fixtures_root.join(fixture_rel);
        let fixture = load_fixture_document(&fixture_abs)
            .unwrap_or_else(|err| panic!("failed to load fixture {fixture_rel}: {err}"));
        let cases = expand_fixture_cases(fixture_rel, fixture);

        for case in &cases {
            let actual_root = parser.parse(&case.input, parse_options.clone());
            let actual = serde_json::to_value(actual_root)
                .expect("failed to serialize parser output to json value");
            let expected = case
                .parse_answer
                .as_ref()
                .expect("fixture case should contain parseAnswer");

            if let Err(diff) = compare_parse_answer(expected, &actual, assert_level) {
                panic!(
                    "fixture gfm subset mismatch\nfixture_path={}\ncase_id={}\n{}",
                    case.fixture_path, case.case_id, diff
                );
            }
        }
    }
}
