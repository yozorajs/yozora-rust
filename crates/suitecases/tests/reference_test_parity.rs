use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use yozora_suitecases::{
    collect_fixture_files, expand_fixture_cases, is_fixture_enabled_for_profile,
    load_fixture_document, parse_profile_map_toml,
};

#[test]
fn every_reference_test_case_has_audited_rust_coverage() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory = read_json(&repo_root.join("test-parity/reference-tests.json"));
    let coverage = read_json(&repo_root.join("test-parity/coverage.json"));

    assert_eq!(inventory["schema_version"], coverage["schema_version"]);
    assert_eq!(inventory["reference_commit"], coverage["reference_commit"]);

    let direct_coverage = coverage["direct_coverage"]
        .as_object()
        .expect("direct_coverage should be an object");
    let reference_hashes = inventory["test_files"]
        .as_array()
        .expect("test_files should be an array")
        .iter()
        .map(|entry| {
            (
                entry["source"].as_str().expect("test file source"),
                entry["sha256"].as_str().expect("test file hash"),
            )
        })
        .collect::<HashMap<_, _>>();
    let not_applicable = coverage["not_applicable"]
        .as_array()
        .expect("not_applicable should be an array");
    let not_applicable_by_id = not_applicable
        .iter()
        .map(|entry| {
            let case_id = entry["case_id"].as_str().expect("N/A case_id");
            let reason = entry["reason"].as_str().expect("N/A reason");
            assert!(!reason.trim().is_empty(), "empty N/A reason for {case_id}");
            (case_id, reason)
        })
        .collect::<HashMap<_, _>>();

    let direct_tests = inventory["direct_tests"]
        .as_array()
        .expect("direct_tests should be an array");
    let direct_cases_by_id = direct_tests
        .iter()
        .map(|entry| {
            (
                entry["id"].as_str().expect("direct test id"),
                entry["source"].as_str().expect("direct test source"),
            )
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(
        direct_cases_by_id.len(),
        direct_tests.len(),
        "duplicate direct test IDs"
    );

    let mut used_sources = HashSet::new();
    let mut covered_case_ids = HashSet::new();
    for (source, mapping) in direct_coverage {
        let reviewed_hash = mapping["reference_sha256"]
            .as_str()
            .expect("reviewed reference hash");
        assert_eq!(
            Some(reviewed_hash),
            reference_hashes.get(source.as_str()).copied(),
            "reference source changed without coverage re-review: {source}"
        );
        let targets = mapping["rust_tests"]
            .as_array()
            .expect("rust_tests should be an array");
        assert!(!targets.is_empty(), "empty coverage targets for {source}");
        for target in targets {
            assert_test_target(&repo_root, target.as_str().expect("coverage target"));
        }

        let case_ids = mapping["case_ids"]
            .as_array()
            .expect("case_ids should be an array");
        assert!(!case_ids.is_empty(), "empty case_ids for {source}");
        for case_id in case_ids {
            let case_id = case_id.as_str().expect("coverage case ID");
            assert_eq!(
                direct_cases_by_id.get(case_id).copied(),
                Some(source.as_str()),
                "coverage case belongs to a different source: {case_id}"
            );
            assert!(
                !not_applicable_by_id.contains_key(case_id),
                "N/A case cannot also have Rust coverage: {case_id}"
            );
            assert!(
                covered_case_ids.insert(case_id),
                "duplicate direct case coverage: {case_id}"
            );
        }
        used_sources.insert(source.as_str());
    }

    let mut covered = 0usize;
    let mut excluded = 0usize;
    for test in direct_tests {
        let case_id = test["id"].as_str().expect("direct test id");
        if not_applicable_by_id.contains_key(case_id) {
            excluded += 1;
            continue;
        }
        assert!(
            covered_case_ids.contains(case_id),
            "missing exact coverage mapping for {case_id}"
        );
        covered += 1;
    }
    for case_id in not_applicable_by_id.keys() {
        assert!(
            direct_cases_by_id.contains_key(case_id),
            "N/A entry does not exist in inventory: {case_id}"
        );
    }
    assert_eq!(covered + excluded, direct_tests.len());

    let fixtures_root = repo_root.join("fixtures");
    let fixture_files = inventory["fixture_files"]
        .as_array()
        .expect("fixture_files should be an array");
    let mut fixture_file_sources = HashSet::new();
    for fixture_file in fixture_files {
        let source = fixture_file["source"]
            .as_str()
            .expect("fixture file source");
        assert!(
            fixture_file_sources.insert(source),
            "duplicate fixture file source: {source}"
        );
        let content = fs::read(fixtures_root.join(source))
            .unwrap_or_else(|error| panic!("failed to read local fixture {source}: {error}"));
        assert_eq!(
            format!("{:016x}", fnv1a64(&content)),
            fixture_file["fnv1a64"]
                .as_str()
                .expect("fixture file FNV-1a hash"),
            "local fixture content differs from reference: {source}"
        );
    }

    let inventory_fixture_ids = inventory["fixture_cases"]
        .as_array()
        .expect("fixture_cases should be an array")
        .iter()
        .map(|entry| entry["id"].as_str().expect("fixture case id").to_string())
        .collect::<HashSet<_>>();
    assert_eq!(
        inventory_fixture_ids.len(),
        inventory["fixture_cases"]
            .as_array()
            .expect("fixture cases")
            .len(),
        "duplicate fixture case IDs"
    );
    let local_fixture_ids = collect_local_fixture_ids(&fixtures_root);
    assert_eq!(local_fixture_ids, inventory_fixture_ids);

    let fixture_not_applicable = coverage["fixture_not_applicable"]
        .as_array()
        .expect("fixture_not_applicable should be an array");
    let fixture_not_applicable_by_id = fixture_not_applicable
        .iter()
        .map(|entry| {
            let case_id = entry["case_id"].as_str().expect("fixture N/A case_id");
            let reason = entry["reason"].as_str().expect("fixture N/A reason");
            assert!(
                !reason.trim().is_empty(),
                "empty fixture N/A reason for {case_id}"
            );
            (case_id, reason)
        })
        .collect::<HashMap<_, _>>();

    let fixture_coverage = coverage["fixture_coverage"]
        .as_object()
        .expect("fixture_coverage should be an object");
    let fixture_scanners = inventory["fixture_scanners"]
        .as_array()
        .expect("fixture_scanners should be an array");
    let scanner_ids = fixture_scanners
        .iter()
        .map(|scanner| scanner["id"].as_str().expect("scanner id"))
        .collect::<HashSet<_>>();
    assert_eq!(
        scanner_ids.len(),
        fixture_scanners.len(),
        "duplicate scanner IDs"
    );
    assert_eq!(
        format!("{:016x}", scanner_inventory_hash(fixture_scanners)),
        fixture_coverage["reviewed_scanners_fnv1a64"]
            .as_str()
            .expect("reviewed scanner hash"),
        "fixture scanner selection changed without coverage re-review"
    );
    for field in ["parse_runner", "markup_runner", "entrypoint"] {
        let target = fixture_coverage[field]
            .as_str()
            .unwrap_or_else(|| panic!("missing fixture coverage field {field}"));
        assert!(
            repo_root.join(target).is_file(),
            "missing fixture target {target}"
        );
    }
    let profiles = fixture_coverage["parse_profiles"]
        .as_array()
        .expect("parse_profiles should be an array")
        .iter()
        .map(|value| value.as_str().expect("profile name"))
        .collect::<HashSet<_>>();
    assert_eq!(
        profiles,
        HashSet::from([
            "gfm",
            "gfm_ex",
            "yozora",
            "yozora_inline_math_backtick_required",
        ])
    );

    let mut reviewed_sources = used_sources;
    reviewed_sources.extend(
        fixture_scanners
            .iter()
            .map(|scanner| scanner["source"].as_str().expect("scanner source")),
    );
    assert_eq!(
        reviewed_sources,
        reference_hashes.keys().copied().collect::<HashSet<_>>(),
        "every reference test file must contain audited direct tests or a fixture scanner"
    );

    let profile_map = parse_profile_map_toml(
        &fs::read_to_string(repo_root.join("fixtures/profile-map.toml")).expect("read profile map"),
    )
    .expect("parse profile map");
    for fixture_case in inventory["fixture_cases"]
        .as_array()
        .expect("fixture_cases should be an array")
    {
        let case_id = fixture_case["id"].as_str().expect("fixture case id");
        let has_parse_answer = fixture_case["has_parse_answer"].as_bool() == Some(true);
        let has_markup_answer = fixture_case["has_markup_answer"].as_bool() == Some(true);
        if !has_parse_answer && !has_markup_answer {
            assert!(
                fixture_not_applicable_by_id.contains_key(case_id),
                "fixture without assertions must be explicitly N/A: {case_id}"
            );
            continue;
        }
        assert!(
            !fixture_not_applicable_by_id.contains_key(case_id),
            "asserted fixture cannot be N/A: {case_id}"
        );
        if !has_parse_answer {
            continue;
        }
        let source = fixture_case["source"].as_str().expect("fixture source");
        assert!(
            profiles.iter().any(|profile| {
                is_fixture_enabled_for_profile(&profile_map, profile, source).unwrap_or(false)
            }),
            "fixture case has no parser profile coverage: {}",
            case_id
        );
    }
    for case_id in fixture_not_applicable_by_id.keys() {
        assert!(
            inventory_fixture_ids.contains(*case_id),
            "fixture N/A entry does not exist in inventory: {case_id}"
        );
    }

    assert_eq!(direct_tests.len(), 481);
    assert_eq!(covered, 474);
    assert_eq!(excluded, 7);
    assert_eq!(inventory_fixture_ids.len(), 746);
    assert_eq!(fixture_not_applicable_by_id.len(), 1);
    assert_eq!(fixture_scanners.len(), 38);
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn assert_test_target(repo_root: &Path, target: &str) {
    let (filepath, test_name) = target
        .rsplit_once("::")
        .unwrap_or_else(|| panic!("coverage target must use path::test_name: {target}"));
    let path = repo_root.join(filepath);
    let content = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("failed to read coverage target {}: {error}", path.display())
    });
    let function_marker = format!("fn {test_name}(");
    let function_index = content
        .find(&function_marker)
        .unwrap_or_else(|| panic!("missing mapped Rust test {target}"));
    let function_line_start = content[..function_index]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let mut is_test = false;
    for line in content[..function_line_start].lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#[") {
            is_test |= line == "#[test]";
            continue;
        }
        break;
    }
    assert!(is_test, "mapped function is not a Rust test: {target}");
}

fn scanner_inventory_hash(scanners: &[Value]) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    for scanner in scanners {
        for field in ["id", "source"] {
            feed_fnv1a64(
                &mut hash,
                scanner[field]
                    .as_str()
                    .unwrap_or_else(|| panic!("scanner {field}"))
                    .as_bytes(),
            );
            feed_fnv1a64(&mut hash, b"\0");
        }
        feed_fnv1a64(
            &mut hash,
            scanner["line"]
                .as_u64()
                .expect("scanner line")
                .to_string()
                .as_bytes(),
        );
        feed_fnv1a64(&mut hash, b"\0");
        feed_fnv1a64(
            &mut hash,
            scanner["expression"]
                .as_str()
                .expect("scanner expression")
                .as_bytes(),
        );
        feed_fnv1a64(&mut hash, b"\0");
    }
    hash
}

fn fnv1a64(value: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    feed_fnv1a64(&mut hash, value);
    hash
}

fn feed_fnv1a64(hash: &mut u64, value: &[u8]) {
    for byte in value {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn collect_local_fixture_ids(fixtures_root: &Path) -> HashSet<String> {
    let files = collect_fixture_files(fixtures_root).expect("collect local fixtures");
    let mut result = HashSet::new();
    for fixture in files {
        let relative = fixture
            .strip_prefix(fixtures_root)
            .expect("fixture below root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "gfm/meta.json" {
            continue;
        }
        let document = load_fixture_document(&fixture).expect("load fixture document");
        for case in expand_fixture_cases(&relative, document) {
            result.insert(case.case_id);
        }
    }
    result
}
