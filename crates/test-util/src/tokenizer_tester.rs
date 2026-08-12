use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use yozora_ast::Root;
use yozora_core_parser::{ParseOptions, Parser};

use crate::{BaseTester, TestFailure, YozoraUseCaseGroup};

pub struct TokenizerTester<P> {
    base: BaseTester,
    pub parser: P,
}

impl<P> TokenizerTester<P>
where
    P: Parser,
{
    pub fn new(case_root_directory: impl Into<PathBuf>, parser: P) -> Self {
        Self {
            base: BaseTester::new(case_root_directory),
            parser,
        }
    }

    pub fn base(&self) -> &BaseTester {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut BaseTester {
        &mut self.base
    }

    pub fn collect(&self) -> Vec<YozoraUseCaseGroup> {
        self.base.collect()
    }

    pub fn reset(&mut self) -> &mut Self {
        self.base.reset();
        self
    }

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
        self.base
            .scan(patterns, case_root_directory, is_desired_filepath)?;
        Ok(self)
    }

    pub fn parse(&self, input: &str, options: Option<ParseOptions>) -> Root {
        self.parser.parse(input, options)
    }

    pub fn run_answer(&mut self) -> Result<(), String> {
        let root = self.base.case_root_directory().to_path_buf();
        let parser = &self.parser;
        for group in self.base.case_groups_mut() {
            answer_group(parser, &root, group)?;
        }
        Ok(())
    }

    pub fn run_test(&self) -> Result<(), Vec<TestFailure>> {
        let mut failures = Vec::new();
        for group in self.base.collect() {
            test_group(&self.parser, &group, &mut failures);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }
}

fn parse_and_format<P>(parser: &P, input: &str) -> Result<Value, String>
where
    P: Parser,
{
    serde_json::to_value(parser.parse(input, None)).map_err(|error| error.to_string())
}

fn answer_group<P>(
    parser: &P,
    parent_dir: &Path,
    group: &mut YozoraUseCaseGroup,
) -> Result<(), String>
where
    P: Parser,
{
    if group.dirpath == group.filepath {
        for subgroup in &mut group.sub_groups {
            answer_group(parser, &group.dirpath, subgroup)?;
        }
        return Ok(());
    }

    for case in &mut group.cases {
        case.parse_answer =
            Some(parse_and_format(parser, &case.input).map_err(|error| {
                format!("[handle failed] {}: {error}", group.filepath.display())
            })?);
    }
    write_group(parent_dir, group)
}

fn test_group<P>(parser: &P, group: &YozoraUseCaseGroup, failures: &mut Vec<TestFailure>)
where
    P: Parser,
{
    for case in &group.cases {
        let actual = parse_and_format(parser, &case.input);
        match (actual, &case.parse_answer) {
            (Ok(actual), Some(expected)) if actual != *expected => failures.push(TestFailure {
                filepath: group.filepath.clone(),
                description: case.description.clone(),
                message: format!("parse answer mismatch\nexpected={expected}\nactual={actual}"),
            }),
            (Ok(_), None) => failures.push(TestFailure {
                filepath: group.filepath.clone(),
                description: case.description.clone(),
                message: "parse answer is missing".to_string(),
            }),
            (Ok(_), Some(_)) => {}
            (Err(message), _) => failures.push(TestFailure {
                filepath: group.filepath.clone(),
                description: case.description.clone(),
                message,
            }),
        }
    }
    for subgroup in &group.sub_groups {
        test_group(parser, subgroup, failures);
    }
}

#[derive(Serialize)]
struct AnswerDocument<'a> {
    title: String,
    cases: &'a [crate::YozoraUseCase],
}

fn write_group(parent_dir: &Path, group: &YozoraUseCaseGroup) -> Result<(), String> {
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
