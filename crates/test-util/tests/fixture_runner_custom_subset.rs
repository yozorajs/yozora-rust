use std::path::PathBuf;

use yozora_parser::YozoraParser;
use yozora_test_util::{
    compare_parse_answer, expand_fixture_cases, load_fixture_document, AssertLevel,
};

#[test]
fn fixture_custom_subset_l1() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_root = repo_root.join("fixtures");
    let fixture_paths = [
        "custom/admonition/basic.json",
        "custom/ecma-import/condition1.json",
        "custom/inline-math/backtick-optional/#001.json",
        "custom/math/single-line/#1.json",
        "custom/footnote/basic.json",
        "custom/footnote-definition/basic2.json",
        "custom/table/backslash.json",
    ];

    let parser = YozoraParser::default();
    for fixture_rel in fixture_paths {
        let fixture_abs = fixtures_root.join(fixture_rel);
        let fixture = load_fixture_document(&fixture_abs)
            .unwrap_or_else(|err| panic!("failed to load fixture {fixture_rel}: {err}"));
        let cases = expand_fixture_cases(fixture_rel, fixture);

        for case in &cases {
            let actual_root = parser.parse(&case.input, None);
            let actual = serde_json::to_value(actual_root)
                .expect("failed to serialize parser output to json value");
            let expected = case
                .parse_answer
                .as_ref()
                .expect("fixture case should contain parseAnswer");

            if let Err(diff) = compare_parse_answer(expected, &actual, AssertLevel::L1) {
                panic!(
                    "fixture custom subset mismatch\nfixture_path={}\ncase_id={}\n{}",
                    case.fixture_path, case.case_id, diff
                );
            }
        }
    }
}
