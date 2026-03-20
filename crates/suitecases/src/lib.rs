use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

mod runner;

pub use runner::{
    format_unexpected_report, run_fixture_batch, run_fixture_subset, SuiteAdapter, SuiteRunOptions,
    SuiteRunReport,
};

#[derive(Debug, Deserialize)]
pub struct FixtureDocument {
    pub title: Option<String>,
    pub cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
pub struct FixtureCase {
    pub description: Option<String>,
    pub input: String,
    #[serde(rename = "parseAnswer")]
    pub parse_answer: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct FixtureCaseRecord {
    pub fixture_path: String,
    pub case_id: String,
    pub index: usize,
    pub description: Option<String>,
    pub input: String,
    pub parse_answer: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssertLevel {
    L0,
    L1,
    L2,
    L3,
}

impl AssertLevel {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "L0" | "l0" => Some(Self::L0),
            "L1" | "l1" => Some(Self::L1),
            "L2" | "l2" => Some(Self::L2),
            "L3" | "l3" => Some(Self::L3),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstDiff {
    pub node_path: String,
    pub expected_excerpt: String,
    pub actual_excerpt: String,
}

impl fmt::Display for AstDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "node_path={}\nexpected={}\nactual={}",
            self.node_path, self.expected_excerpt, self.actual_excerpt
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRule {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileMap {
    pub rules: HashMap<String, ProfileRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFailure {
    pub case_id: String,
    pub parser_profile: String,
    pub assert_level: AssertLevel,
    pub reason: String,
    pub issue: String,
    pub introduced_in: String,
    pub expires_in_milestone: String,
}

pub fn load_fixture_document(path: &Path) -> Result<FixtureDocument, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read fixture {}: {err}", path.display()))?;
    serde_json::from_str::<FixtureDocument>(&content)
        .map_err(|err| format!("failed to parse fixture {}: {err}", path.display()))
}

pub fn expand_fixture_cases(
    fixture_path: &str,
    document: FixtureDocument,
) -> Vec<FixtureCaseRecord> {
    document
        .cases
        .into_iter()
        .enumerate()
        .map(|(index, case)| {
            let slug = slugify_case_description(case.description.as_deref(), index);
            FixtureCaseRecord {
                fixture_path: fixture_path.to_string(),
                case_id: format!("{fixture_path}::{index}::{slug}"),
                index,
                description: case.description,
                input: case.input,
                parse_answer: case.parse_answer,
            }
        })
        .collect()
}

pub fn parse_profile_map_toml(input: &str) -> Result<ProfileMap, String> {
    let mut rules = HashMap::new();
    let lines = collect_clean_lines(input);
    let mut index = 0usize;
    let mut current_section: Option<String> = None;

    while index < lines.len() {
        let line = &lines[index];
        if line.starts_with('[') && line.ends_with(']') {
            let section = line.trim_start_matches('[').trim_end_matches(']');
            if section.is_empty() {
                return Err("empty profile section in profile-map.toml".to_string());
            }
            let section_key = section.to_string();
            rules.entry(section_key.clone()).or_insert(ProfileRule {
                include: Vec::new(),
                exclude: Vec::new(),
            });
            current_section = Some(section_key);
            index += 1;
            continue;
        }

        let Some(section) = current_section.as_deref() else {
            return Err(format!(
                "unexpected line before profile section: {}",
                lines[index]
            ));
        };

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "invalid key-value line in profile-map.toml: {line}"
            ));
        };

        let key = key.trim();
        let value = value.trim();
        let parsed = parse_toml_string_array(&lines, &mut index, value)?;
        let rule = rules
            .get_mut(section)
            .ok_or_else(|| "internal error: missing profile rule section".to_string())?;

        match key {
            "include" => rule.include = parsed,
            "exclude" => rule.exclude = parsed,
            _ => {
                return Err(format!(
                    "unsupported key in profile-map.toml section [{section}]: {key}"
                ))
            }
        }

        index += 1;
    }

    Ok(ProfileMap { rules })
}

pub fn parse_known_failures_toml(input: &str) -> Result<Vec<KnownFailure>, String> {
    let lines = collect_clean_lines(input);
    let mut failures = Vec::new();
    let mut current: HashMap<String, String> = HashMap::new();

    for line in &lines {
        if line == "[[failure]]" {
            if !current.is_empty() {
                failures.push(build_known_failure(&current)?);
                current.clear();
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "invalid key-value line in known-failures.toml: {line}"
            ));
        };
        current.insert(
            key.trim().to_string(),
            parse_toml_quoted_string(value.trim())?,
        );
    }

    if !current.is_empty() {
        failures.push(build_known_failure(&current)?);
    }

    Ok(failures)
}

pub fn is_fixture_enabled_for_profile(
    profile_map: &ProfileMap,
    profile: &str,
    fixture_path: &str,
) -> Result<bool, String> {
    let rule = profile_map
        .rules
        .get(profile)
        .ok_or_else(|| format!("profile `{profile}` not found in profile-map.toml"))?;
    Ok(matches_rule(rule, fixture_path))
}

pub fn match_fixture_profile(
    profile_map: &ProfileMap,
    fixture_path: &str,
) -> Result<String, String> {
    let matched: Vec<String> = profile_map
        .rules
        .iter()
        .filter_map(|(profile, rule)| {
            if matches_rule(rule, fixture_path) {
                Some(profile.clone())
            } else {
                None
            }
        })
        .collect();

    match matched.len() {
        0 => Err(format!(
            "fixture `{fixture_path}` does not match any parser profile"
        )),
        1 => Ok(matched[0].clone()),
        _ => Err(format!(
            "fixture `{fixture_path}` matches multiple parser profiles: {}",
            matched.join(", ")
        )),
    }
}

pub fn is_known_failure(
    known_failures: &[KnownFailure],
    case_id: &str,
    parser_profile: &str,
    assert_level: AssertLevel,
) -> bool {
    known_failures.iter().any(|entry| {
        entry.case_id == case_id
            && entry.parser_profile == parser_profile
            && entry.assert_level == assert_level
    })
}

pub fn compare_parse_answer(
    expected: &Value,
    actual: &Value,
    assert_level: AssertLevel,
) -> Result<(), AstDiff> {
    let ignore_position = matches!(assert_level, AssertLevel::L1);
    let expected = normalize_ast_value(expected, ignore_position);
    let actual = normalize_ast_value(actual, ignore_position);

    match diff_value(&expected, &actual, "root") {
        Some(diff) => Err(diff),
        None => Ok(()),
    }
}

pub fn collect_fixture_files(fixtures_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    visit_json_files(fixtures_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_clean_lines(input: &str) -> Vec<String> {
    input
        .lines()
        .map(strip_toml_comment)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn strip_toml_comment(line: &str) -> String {
    let mut escaped = false;
    let mut in_quote = false;

    for (idx, ch) in line.char_indices() {
        match ch {
            '\\' if in_quote && !escaped => {
                escaped = true;
            }
            '"' if !escaped => {
                in_quote = !in_quote;
            }
            '#' if !in_quote => {
                return line[..idx].to_string();
            }
            _ => {
                escaped = false;
            }
        }
    }

    line.to_string()
}

fn parse_toml_string_array(
    lines: &[String],
    index: &mut usize,
    value: &str,
) -> Result<Vec<String>, String> {
    if !value.starts_with('[') {
        return Err(format!("array should start with `[`: {value}"));
    }

    let mut buf = String::from(value);
    while !buf.contains(']') {
        *index += 1;
        if *index >= lines.len() {
            return Err("unterminated array in toml".to_string());
        }
        buf.push(' ');
        buf.push_str(lines[*index].trim());
    }

    parse_toml_string_list_from_array(&buf)
}

fn parse_toml_string_list_from_array(array: &str) -> Result<Vec<String>, String> {
    let start = array
        .find('[')
        .ok_or_else(|| "array missing `[`".to_string())?;
    let end = array
        .rfind(']')
        .ok_or_else(|| "array missing `]`".to_string())?;
    let body = &array[start + 1..end];
    let mut values = Vec::new();
    let mut chars = body.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_whitespace() || ch == ',' {
            continue;
        }

        if ch != '"' {
            return Err(format!("array item must be quoted string: {array}"));
        }

        let mut escaped = false;
        let mut item = String::new();
        for c in chars.by_ref() {
            if escaped {
                let unescaped = match c {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                };
                item.push(unescaped);
                escaped = false;
                continue;
            }

            if c == '\\' {
                escaped = true;
                continue;
            }

            if c == '"' {
                break;
            }

            item.push(c);
        }

        values.push(item);
    }

    Ok(values)
}

fn parse_toml_quoted_string(value: &str) -> Result<String, String> {
    if !(value.starts_with('"') && value.ends_with('"')) {
        return Err(format!("value should be quoted string: {value}"));
    }

    let mut result = String::new();
    let mut escaped = false;
    for ch in value[1..value.len() - 1].chars() {
        if escaped {
            result.push(match ch {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        result.push(ch);
    }

    Ok(result)
}

fn build_known_failure(values: &HashMap<String, String>) -> Result<KnownFailure, String> {
    let case_id = required_field(values, "case_id")?;
    let parser_profile = required_field(values, "parser_profile")?;
    let level_str = required_field(values, "assert_level")?;
    let assert_level = AssertLevel::from_str(&level_str)
        .ok_or_else(|| format!("invalid assert_level `{level_str}` in known-failures.toml"))?;

    Ok(KnownFailure {
        case_id,
        parser_profile,
        assert_level,
        reason: required_field(values, "reason")?,
        issue: required_field(values, "issue")?,
        introduced_in: required_field(values, "introduced_in")?,
        expires_in_milestone: required_field(values, "expires_in_milestone")?,
    })
}

fn required_field(values: &HashMap<String, String>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing `{key}` in known-failures entry"))
}

fn matches_rule(rule: &ProfileRule, fixture_path: &str) -> bool {
    let normalized = normalize_path(fixture_path);
    let include_hit = rule
        .include
        .iter()
        .any(|pattern| glob_match(pattern, &normalized));
    if !include_hit {
        return false;
    }

    let exclude_hit = rule
        .exclude
        .iter()
        .any(|pattern| glob_match(pattern, &normalized));
    !exclude_hit
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn normalize_glob_pattern(pattern: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for ch in pattern.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        out.push(ch);
    }
    out
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let normalized = normalize_glob_pattern(pattern);
    let path = normalize_path(path);

    let pattern_segments: Vec<&str> = normalized.split('/').collect();
    let path_segments: Vec<&str> = path.split('/').collect();
    glob_match_segments(&pattern_segments, &path_segments, 0, 0)
}

fn glob_match_segments(pattern: &[&str], path: &[&str], p_idx: usize, s_idx: usize) -> bool {
    if p_idx == pattern.len() {
        return s_idx == path.len();
    }

    if pattern[p_idx] == "**" {
        for k in s_idx..=path.len() {
            if glob_match_segments(pattern, path, p_idx + 1, k) {
                return true;
            }
        }
        return false;
    }

    if s_idx >= path.len() {
        return false;
    }

    if segment_match(pattern[p_idx], path[s_idx]) {
        return glob_match_segments(pattern, path, p_idx + 1, s_idx + 1);
    }

    false
}

fn segment_match(pattern: &str, value: &str) -> bool {
    let p_chars: Vec<char> = pattern.chars().collect();
    let v_chars: Vec<char> = value.chars().collect();

    let mut p = 0usize;
    let mut v = 0usize;
    let mut star_idx: Option<usize> = None;
    let mut v_checkpoint = 0usize;

    while v < v_chars.len() {
        if p < p_chars.len() && (p_chars[p] == '?' || p_chars[p] == v_chars[v]) {
            p += 1;
            v += 1;
            continue;
        }

        if p < p_chars.len() && p_chars[p] == '*' {
            star_idx = Some(p);
            p += 1;
            v_checkpoint = v;
            continue;
        }

        if let Some(star_pos) = star_idx {
            p = star_pos + 1;
            v_checkpoint += 1;
            v = v_checkpoint;
            continue;
        }

        return false;
    }

    while p < p_chars.len() && p_chars[p] == '*' {
        p += 1;
    }

    p == p_chars.len()
}

fn slugify_case_description(description: Option<&str>, index: usize) -> String {
    let raw = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("case");

    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        format!("case-{index}")
    } else {
        slug
    }
}

fn normalize_ast_value(value: &Value, ignore_position: bool) -> Value {
    match value {
        Value::Object(obj) => {
            let mut sorted_keys = BTreeSet::new();
            for key in obj.keys() {
                if ignore_position && key == "position" {
                    continue;
                }
                sorted_keys.insert(key.clone());
            }

            let mut normalized = serde_json::Map::new();
            for key in sorted_keys {
                let item = normalize_ast_value(
                    obj.get(&key)
                        .expect("normalized key should exist in source object"),
                    ignore_position,
                );
                if item.is_null() {
                    continue;
                }
                normalized.insert(key, item);
            }
            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| normalize_ast_value(item, ignore_position))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn diff_value(expected: &Value, actual: &Value, path: &str) -> Option<AstDiff> {
    match (expected, actual) {
        (Value::Null, Value::Null)
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Number(_), Value::Number(_))
        | (Value::String(_), Value::String(_)) => {
            if expected == actual {
                None
            } else {
                Some(make_diff(path, expected, actual))
            }
        }
        (Value::Array(expected), Value::Array(actual)) => {
            if expected.len() != actual.len() {
                return Some(make_diff(path, expected, actual));
            }
            for (index, (lhs, rhs)) in expected.iter().zip(actual.iter()).enumerate() {
                let next_path = format!("{path}[{index}]");
                if let Some(diff) = diff_value(lhs, rhs, &next_path) {
                    return Some(diff);
                }
            }
            None
        }
        (Value::Object(expected), Value::Object(actual)) => {
            if expected.len() != actual.len() {
                return Some(make_diff(path, expected, actual));
            }

            for (key, lhs) in expected {
                let Some(rhs) = actual.get(key) else {
                    return Some(make_diff(path, expected, actual));
                };
                let next_path = format!("{path}.{key}");
                if let Some(diff) = diff_value(lhs, rhs, &next_path) {
                    return Some(diff);
                }
            }

            None
        }
        _ => Some(make_diff(path, expected, actual)),
    }
}

fn make_diff(
    path: &str,
    expected: &impl serde::Serialize,
    actual: &impl serde::Serialize,
) -> AstDiff {
    AstDiff {
        node_path: path.to_string(),
        expected_excerpt: serde_json::to_string(expected)
            .unwrap_or_else(|_| "<serialize-error>".to_string()),
        actual_excerpt: serde_json::to_string(actual)
            .unwrap_or_else(|_| "<serialize-error>".to_string()),
    }
}

fn visit_json_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("failed to read fixtures dir {}: {err}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read entry in fixtures dir {}: {err}",
                dir.display()
            )
        })?;

        let path = entry.path();
        if path.is_dir() {
            visit_json_files(&path, out)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            out.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_l1_ignores_position() {
        let expected = serde_json::json!({
            "type": "root",
            "position": { "start": { "line": 1 }, "end": { "line": 1 } },
            "children": [
                {
                    "type": "paragraph",
                    "position": { "start": { "line": 1 }, "end": { "line": 1 } },
                    "children": [{ "type": "text", "value": "a" }]
                }
            ]
        });
        let actual = serde_json::json!({
            "type": "root",
            "position": { "start": { "line": 9 }, "end": { "line": 9 } },
            "children": [
                {
                    "type": "paragraph",
                    "position": { "start": { "line": 9 }, "end": { "line": 9 } },
                    "children": [{ "type": "text", "value": "a" }]
                }
            ]
        });

        assert!(compare_parse_answer(&expected, &actual, AssertLevel::L1).is_ok());
    }

    #[test]
    fn compare_l2_checks_position() {
        let expected =
            serde_json::json!({ "type": "root", "position": { "start": { "line": 1 } } });
        let actual = serde_json::json!({ "type": "root", "position": { "start": { "line": 2 } } });

        assert!(compare_parse_answer(&expected, &actual, AssertLevel::L2).is_err());
    }

    #[test]
    fn compare_reports_first_node_path() {
        let expected = serde_json::json!({
            "type": "root",
            "children": [{ "type": "paragraph", "children": [{ "type": "text", "value": "x" }] }]
        });
        let actual = serde_json::json!({
            "type": "root",
            "children": [{ "type": "paragraph", "children": [{ "type": "text", "value": "y" }] }]
        });

        let diff = compare_parse_answer(&expected, &actual, AssertLevel::L1)
            .expect_err("should find mismatch in text value");
        assert_eq!(diff.node_path, "root.children[0].children[0].value");
    }

    #[test]
    fn parse_profile_map_and_match() {
        let text = r#"
        [gfm]
        include = ["gfm/**/*.json"]
        exclude = ["gfm/table/**/*", "gfm/list-item/task list items\\(extension\\)/**/*"]

        [yozora]
        include = ["custom/**/*.json", "gfm/**/*.json"]
        exclude = []
        "#;

        let map = parse_profile_map_toml(text).expect("profile map should parse");
        let gfm_hit = is_fixture_enabled_for_profile(&map, "gfm", "gfm/emphasis/rule#1/#360.json")
            .expect("gfm profile should exist");
        assert!(gfm_hit);

        let gfm_table_hit = is_fixture_enabled_for_profile(&map, "gfm", "gfm/table/#200.json")
            .expect("gfm profile should exist");
        assert!(!gfm_table_hit);

        let gfm_task_hit = is_fixture_enabled_for_profile(
            &map,
            "gfm",
            "gfm/list-item/task list items(extension)/#279.json",
        )
        .expect("gfm profile should exist");
        assert!(!gfm_task_hit);
    }

    #[test]
    fn parse_known_failures() {
        let text = r#"
        [[failure]]
        case_id = "gfm/emphasis/rule#1/#360.json::0::rule-1"
        parser_profile = "gfm"
        assert_level = "L2"
        reason = "Position offset mismatch"
        issue = "https://example.com/issue/1"
        introduced_in = "M2"
        expires_in_milestone = "M4"
        "#;

        let failures = parse_known_failures_toml(text).expect("known failures should parse");
        assert_eq!(failures.len(), 1);
        assert!(is_known_failure(
            &failures,
            "gfm/emphasis/rule#1/#360.json::0::rule-1",
            "gfm",
            AssertLevel::L2
        ));
    }

    #[test]
    fn expand_fixture_case_id_with_slug() {
        let doc = FixtureDocument {
            title: Some("t".to_string()),
            cases: vec![FixtureCase {
                description: Some("Rule 1".to_string()),
                input: "*foo*".to_string(),
                parse_answer: None,
            }],
        };

        let records = expand_fixture_cases("gfm/emphasis/rule#1/#360.json", doc);
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].case_id,
            "gfm/emphasis/rule#1/#360.json::0::rule-1"
        );
    }
}
