use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{
    collect_fixture_files, compare_parse_answer, expand_fixture_cases,
    is_fixture_enabled_for_profile, is_known_failure, load_fixture_document, AssertLevel,
    KnownFailure, ProfileMap,
};

pub trait SuiteAdapter {
    fn name(&self) -> &str;

    fn run_case(&self, input: &str) -> Result<Value, String>;
}

#[derive(Debug, Clone)]
pub struct SuiteRunOptions<'a> {
    pub fixtures_root: &'a Path,
    pub parser_profile: &'a str,
    pub assert_level: AssertLevel,
    pub profile_map: Option<&'a ProfileMap>,
    pub known_failures: &'a [KnownFailure],
}

#[derive(Debug, Clone, Default)]
pub struct SuiteRunReport {
    pub total_cases: usize,
    pub known_failure_cases: usize,
    pub skipped_cases: usize,
    pub unexpected: Vec<String>,
}

impl SuiteRunReport {
    pub fn merge(&mut self, other: SuiteRunReport) {
        self.total_cases += other.total_cases;
        self.known_failure_cases += other.known_failure_cases;
        self.skipped_cases += other.skipped_cases;
        self.unexpected.extend(other.unexpected);
    }

    pub fn is_clean(&self) -> bool {
        self.unexpected.is_empty()
    }
}

pub fn run_fixture_subset(
    adapter: &dyn SuiteAdapter,
    options: &SuiteRunOptions<'_>,
    fixture_rels: &[&str],
) -> SuiteRunReport {
    let mut report = SuiteRunReport::default();

    for fixture_rel in fixture_rels {
        if !is_enabled_for_profile(options, fixture_rel) {
            continue;
        }

        let fixture_abs = options.fixtures_root.join(fixture_rel);
        let fixture_report = run_single_fixture(adapter, options, fixture_rel, &fixture_abs);
        report.merge(fixture_report);
    }

    report
}

pub fn run_fixture_batch(
    adapter: &dyn SuiteAdapter,
    options: &SuiteRunOptions<'_>,
) -> Result<SuiteRunReport, String> {
    let fixture_files = collect_fixture_files(options.fixtures_root)?;
    let mut report = SuiteRunReport::default();

    for fixture_abs in &fixture_files {
        let fixture_rel = rel_path(options.fixtures_root, fixture_abs)
            .map_err(|err| format!("invalid fixture path {}: {err}", fixture_abs.display()))?;
        if !is_enabled_for_profile(options, &fixture_rel) {
            continue;
        }

        let fixture_report = run_single_fixture(adapter, options, &fixture_rel, fixture_abs);
        report.merge(fixture_report);
    }

    Ok(report)
}

pub fn format_unexpected_report(report: &SuiteRunReport, max_print: usize) -> String {
    if report.unexpected.is_empty() {
        return format!(
            "suite report: total_cases={} known_failures={} skipped_cases={} unexpected=0",
            report.total_cases, report.known_failure_cases, report.skipped_cases,
        );
    }

    let mut out = String::new();
    out.push_str(&format!(
        "suite report: total_cases={} known_failures={} skipped_cases={} unexpected={} (showing up to {})\n",
        report.total_cases,
        report.known_failure_cases,
        report.skipped_cases,
        report.unexpected.len(),
        max_print,
    ));

    for item in report.unexpected.iter().take(max_print) {
        out.push_str(item);
        out.push('\n');
    }

    out
}

fn run_single_fixture(
    adapter: &dyn SuiteAdapter,
    options: &SuiteRunOptions<'_>,
    fixture_rel: &str,
    fixture_abs: &Path,
) -> SuiteRunReport {
    let mut report = SuiteRunReport::default();

    let doc = match load_fixture_document(fixture_abs) {
        Ok(doc) => doc,
        Err(err) => {
            report
                .unexpected
                .push(format!("[load] fixture={fixture_rel} error={err}"));
            return report;
        }
    };

    let cases = expand_fixture_cases(fixture_rel, doc);
    for case in &cases {
        report.total_cases += 1;

        let actual = match adapter.run_case(&case.input) {
            Ok(value) => value,
            Err(err) => {
                report.unexpected.push(format!(
                    "[run] fixture={} case_id={} adapter={} error={}",
                    case.fixture_path,
                    case.case_id,
                    adapter.name(),
                    err,
                ));
                continue;
            }
        };

        let Some(expected) = case.parse_answer.as_ref() else {
            report.skipped_cases += 1;
            continue;
        };

        if let Err(diff) = compare_parse_answer(expected, &actual, options.assert_level) {
            if is_known_failure(
                options.known_failures,
                &case.case_id,
                options.parser_profile,
                options.assert_level,
            ) {
                report.known_failure_cases += 1;
            } else {
                report.unexpected.push(format!(
                    "fixture={} case_id={} profile={} level={:?} {}",
                    case.fixture_path,
                    case.case_id,
                    options.parser_profile,
                    options.assert_level,
                    diff,
                ));
            }
        }
    }

    report
}

fn is_enabled_for_profile(options: &SuiteRunOptions<'_>, fixture_rel: &str) -> bool {
    let Some(profile_map) = options.profile_map else {
        return true;
    };
    is_fixture_enabled_for_profile(profile_map, options.parser_profile, fixture_rel)
        .unwrap_or(false)
}

fn rel_path(root: &Path, abs: &Path) -> Result<String, String> {
    let rel: PathBuf = abs
        .strip_prefix(root)
        .map_err(|err| err.to_string())?
        .to_path_buf();

    Ok(rel.to_string_lossy().replace('\\', "/"))
}
