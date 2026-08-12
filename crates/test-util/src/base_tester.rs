use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::{TestFailure, YozoraUseCase, YozoraUseCaseGroup};

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
struct CaseDocument<T> {
    title: Option<String>,
    #[serde(default)]
    cases: Vec<YozoraUseCase<T>>,
}

pub struct BaseTester<T = Value> {
    case_root_directory: PathBuf,
    case_groups: Vec<YozoraUseCaseGroup<T>>,
    visited_filepath_set: HashSet<PathBuf>,
}

impl<T> BaseTester<T> {
    pub fn new(case_root_directory: impl Into<PathBuf>) -> Self {
        Self {
            case_root_directory: normalize_path(case_root_directory.into()),
            case_groups: Vec::new(),
            visited_filepath_set: HashSet::new(),
        }
    }

    pub fn case_root_directory(&self) -> &Path {
        &self.case_root_directory
    }

    pub fn reset(&mut self) -> &mut Self {
        self.case_groups.clear();
        self.visited_filepath_set.clear();
        self
    }

    pub fn careful_process<R, F>(&self, filepath: &Path, process: F) -> Result<R, String>
    where
        F: FnOnce() -> Result<R, String>,
    {
        process().map_err(|error| format!("[handle failed] {}: {error}", filepath.display()))
    }

    pub fn stringify<S>(&self, data: &S) -> Result<String, String>
    where
        S: Serialize,
    {
        serde_json::to_string_pretty(data).map_err(|error| error.to_string())
    }

    pub fn format<S>(&self, data: &S) -> Result<Value, String>
    where
        S: Serialize,
    {
        serde_json::to_value(data).map_err(|error| error.to_string())
    }
}

impl<T> BaseTester<T>
where
    T: Clone,
{
    pub fn collect(&self) -> Vec<YozoraUseCaseGroup<T>> {
        self.case_groups.clone()
    }

    pub fn run_test<F>(&self, mut test_case: F) -> Result<(), Vec<TestFailure>>
    where
        F: FnMut(&YozoraUseCase<T>, &Path) -> Vec<TestFailure>,
    {
        let mut failures = Vec::new();
        for group in &self.case_groups {
            test_group(group, &mut test_case, &mut failures);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }
}

impl<T> BaseTester<T>
where
    T: Clone + Serialize,
{
    pub fn run_answer<F>(&mut self, mut answer_case: F) -> Result<(), String>
    where
        F: FnMut(&mut YozoraUseCase<T>, &Path) -> Result<(), String>,
    {
        let root = self.case_root_directory.clone();
        for group in &mut self.case_groups {
            answer_group(&root, group, &mut answer_case)?;
        }
        Ok(())
    }
}

impl<T> BaseTester<T>
where
    T: Clone + DeserializeOwned,
{
    pub fn scan<I, S, F>(
        &mut self,
        patterns: I,
        case_root_directory: Option<&Path>,
        is_desired_filepath: F,
    ) -> Result<&mut Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&Path) -> bool,
    {
        let case_root_directory = normalize_path(
            case_root_directory
                .unwrap_or(&self.case_root_directory)
                .to_path_buf(),
        );
        let mut include_patterns = Vec::new();
        let mut exclude_patterns = Vec::new();
        for pattern in patterns {
            let pattern = normalize_pattern_path(pattern.as_ref());
            if let Some(pattern) = pattern.strip_prefix('!') {
                exclude_patterns.push(pattern.to_string());
            } else {
                include_patterns.push(pattern);
            }
        }

        let mut filepaths = collect_filepaths(&case_root_directory)?;
        filepaths.retain(|filepath| {
            let relative = filepath
                .strip_prefix(&case_root_directory)
                .unwrap_or(filepath)
                .to_string_lossy()
                .replace('\\', "/");
            let included = include_patterns.is_empty()
                || include_patterns
                    .iter()
                    .any(|pattern| path_matches(pattern, &relative));
            included
                && !exclude_patterns
                    .iter()
                    .any(|pattern| path_matches(pattern, &relative))
        });
        filepaths.sort();

        for filepath in filepaths {
            if is_desired_filepath(&filepath) {
                self.scan_for_use_case_group(&filepath)?;
            }
        }
        Ok(self)
    }

    pub fn scan_all(&mut self) -> Result<&mut Self, String> {
        self.scan(["**"], None, |_| true)
    }

    fn scan_for_use_case_group(&mut self, filepath: &Path) -> Result<(), String> {
        let filepath = normalize_path(filepath.to_path_buf());
        if !self.visited_filepath_set.insert(filepath.clone()) {
            return Ok(());
        }

        let content = fs::read_to_string(&filepath)
            .map_err(|error| format!("failed to read {}: {error}", filepath.display()))?;
        let mut document: CaseDocument<T> = serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", filepath.display()))?;
        for (index, case) in document.cases.iter_mut().enumerate() {
            if case.description.is_empty() {
                case.description = format!("case#{index}");
            }
        }

        let dirpath = filepath
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.case_root_directory.clone());
        let group = YozoraUseCaseGroup {
            dirpath: dirpath.clone(),
            filepath,
            title: document.title,
            cases: document.cases,
            sub_groups: Vec::new(),
        };
        insert_group(
            &mut self.case_groups,
            &self.case_root_directory,
            &dirpath,
            group,
        );
        Ok(())
    }
}

fn insert_group<T>(
    groups: &mut Vec<YozoraUseCaseGroup<T>>,
    root: &Path,
    dirpath: &Path,
    group: YozoraUseCaseGroup<T>,
) {
    let Ok(relative_dir) = dirpath.strip_prefix(root) else {
        groups.push(group);
        return;
    };
    let components = relative_dir
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.is_empty() {
        groups.push(group);
        return;
    }

    let mut current_groups = groups;
    let mut current_path = root.to_path_buf();
    for component in components {
        current_path.push(component);
        let index = current_groups
            .iter()
            .position(|candidate| {
                candidate.dirpath == current_path && candidate.filepath == current_path
            })
            .unwrap_or_else(|| {
                current_groups.push(YozoraUseCaseGroup {
                    dirpath: current_path.clone(),
                    filepath: current_path.clone(),
                    title: None,
                    cases: Vec::new(),
                    sub_groups: Vec::new(),
                });
                current_groups.len() - 1
            });
        current_groups = &mut current_groups[index].sub_groups;
    }
    current_groups.push(group);
}

fn test_group<T, F>(
    group: &YozoraUseCaseGroup<T>,
    test_case: &mut F,
    failures: &mut Vec<TestFailure>,
) where
    F: FnMut(&YozoraUseCase<T>, &Path) -> Vec<TestFailure>,
{
    for case in &group.cases {
        failures.extend(test_case(case, &group.filepath));
    }
    for subgroup in &group.sub_groups {
        test_group(subgroup, test_case, failures);
    }
}

fn answer_group<T, F>(
    parent_dir: &Path,
    group: &mut YozoraUseCaseGroup<T>,
    answer_case: &mut F,
) -> Result<(), String>
where
    T: Serialize,
    F: FnMut(&mut YozoraUseCase<T>, &Path) -> Result<(), String>,
{
    if group.dirpath == group.filepath {
        for subgroup in &mut group.sub_groups {
            answer_group(&group.dirpath, subgroup, answer_case)?;
        }
        return Ok(());
    }

    for case in &mut group.cases {
        answer_case(case, &group.filepath)?;
    }

    let title = group.title.clone().unwrap_or_else(|| {
        group
            .dirpath
            .strip_prefix(parent_dir)
            .unwrap_or(&group.dirpath)
            .to_string_lossy()
            .to_string()
    });
    let content = serde_json::to_string_pretty(&AnswerDocument {
        title,
        cases: &group.cases,
    })
    .map_err(|error| error.to_string())?;
    fs::write(&group.filepath, format!("{content}\n"))
        .map_err(|error| format!("failed to write {}: {error}", group.filepath.display()))
}

#[derive(Serialize)]
struct AnswerDocument<'a, T> {
    title: String,
    cases: &'a [YozoraUseCase<T>],
}

fn collect_filepaths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut queue = vec![root.to_path_buf()];
    let mut filepaths = Vec::new();
    while let Some(directory) = queue.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            let filepath = normalize_path(entry.path());
            if file_type.is_dir() {
                queue.push(filepath);
            } else if file_type.is_file() {
                filepaths.push(filepath);
            }
        }
    }
    Ok(filepaths)
}

fn normalize_path(path: PathBuf) -> PathBuf {
    path.components().collect()
}

fn normalize_pattern_path(pattern: &str) -> String {
    pattern
        .trim_start_matches("./")
        .trim_end_matches('/')
        .replace('\\', "/")
}

fn path_matches(pattern: &str, relative_filepath: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if !has_glob_magic(pattern) {
        return relative_filepath == pattern
            || relative_filepath
                .strip_prefix(pattern)
                .is_some_and(|suffix| suffix.starts_with('/'));
    }

    let pattern_segments = split_pattern_segments(pattern);
    let path_segments = relative_filepath.split('/').collect::<Vec<_>>();
    match_segments(&pattern_segments, &path_segments, 0, 0)
}

fn has_glob_magic(pattern: &str) -> bool {
    let mut escaped = false;
    for character in pattern.chars() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if matches!(character, '*' | '?' | '[' | '{') {
            return true;
        }
    }
    false
}

fn split_pattern_segments(pattern: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut segment = String::new();
    let mut escaped = false;
    for character in pattern.chars() {
        if escaped {
            segment.push('\\');
            segment.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '/' {
            segments.push(std::mem::take(&mut segment));
        } else {
            segment.push(character);
        }
    }
    if escaped {
        segment.push('\\');
    }
    segments.push(segment);
    segments
}

fn match_segments(
    patterns: &[String],
    paths: &[&str],
    pattern_index: usize,
    path_index: usize,
) -> bool {
    if pattern_index >= patterns.len() {
        return path_index >= paths.len();
    }
    if patterns[pattern_index] == "**" {
        return (path_index..=paths.len()).any(|next_path_index| {
            match_segments(patterns, paths, pattern_index + 1, next_path_index)
        });
    }
    path_index < paths.len()
        && segment_matches(&patterns[pattern_index], paths[path_index])
        && match_segments(patterns, paths, pattern_index + 1, path_index + 1)
}

fn segment_matches(pattern: &str, value: &str) -> bool {
    fn matches_from(
        pattern: &[char],
        value: &[char],
        pattern_index: usize,
        value_index: usize,
    ) -> bool {
        if pattern_index >= pattern.len() {
            return value_index >= value.len();
        }
        match pattern[pattern_index] {
            '\\' if pattern_index + 1 < pattern.len() => {
                value_index < value.len()
                    && pattern[pattern_index + 1] == value[value_index]
                    && matches_from(pattern, value, pattern_index + 2, value_index + 1)
            }
            '*' => (value_index..=value.len()).any(|next_value_index| {
                matches_from(pattern, value, pattern_index + 1, next_value_index)
            }),
            '?' => {
                value_index < value.len()
                    && matches_from(pattern, value, pattern_index + 1, value_index + 1)
            }
            character => {
                value_index < value.len()
                    && character == value[value_index]
                    && matches_from(pattern, value, pattern_index + 1, value_index + 1)
            }
        }
    }

    matches_from(
        &pattern.chars().collect::<Vec<_>>(),
        &value.chars().collect::<Vec<_>>(),
        0,
        0,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Value;

    use super::{path_matches, BaseTester};

    #[test]
    fn path_matcher_matches_upstream_glob_semantics() {
        assert!(path_matches("**/*.json", "a/b/c.json"));
        assert!(path_matches("a/?/c.json", "a/b/c.json"));
        assert!(path_matches("a", "a/b.json"));
        assert!(!path_matches("a/*.json", "a/b/c.json"));
    }

    #[test]
    fn scan_groups_cases_and_applies_excludes() {
        let root = std::env::temp_dir().join(format!(
            "yozora-test-util-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("scan")
        ));
        let included_dir = root.join("included");
        let excluded_dir = root.join("excluded");
        fs::create_dir_all(&included_dir).expect("create included directory");
        fs::create_dir_all(&excluded_dir).expect("create excluded directory");
        fs::write(
            included_dir.join("case.json"),
            r#"{"title":"fixture","cases":[{"input":"x","parseAnswer":null}]}"#,
        )
        .expect("write included fixture");
        fs::write(
            excluded_dir.join("case.json"),
            r#"{"title":"excluded","cases":[{"input":"y"}]}"#,
        )
        .expect("write excluded fixture");

        let mut tester = BaseTester::<Value>::new(&root);
        tester
            .scan(["**/*.json", "!excluded/**"], None, |_| true)
            .expect("scan fixtures");
        let groups = tester.collect();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].sub_groups.len(), 1);
        assert_eq!(groups[0].sub_groups[0].cases[0].description, "case#0");

        fs::remove_dir_all(root).expect("remove fixtures");
    }
}
