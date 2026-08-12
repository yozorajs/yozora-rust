use std::path::{Path, PathBuf};

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
        let parser = &self.parser;
        self.base.run_answer(|case, filepath| {
            case.parse_answer = Some(
                parse_and_format(parser, &case.input)
                    .map_err(|error| format!("[handle failed] {}: {error}", filepath.display()))?,
            );
            Ok(())
        })
    }

    pub fn run_test(&self) -> Result<(), Vec<TestFailure>> {
        self.base.run_test(|case, filepath| {
            let mut failures = Vec::new();
            match (
                parse_and_format(&self.parser, &case.input),
                &case.parse_answer,
            ) {
                (Ok(actual), Some(expected)) if actual != *expected => failures.push(TestFailure {
                    filepath: filepath.to_path_buf(),
                    description: case.description.clone(),
                    message: format!("parse answer mismatch\nexpected={expected}\nactual={actual}"),
                }),
                (Ok(_), None) => failures.push(TestFailure {
                    filepath: filepath.to_path_buf(),
                    description: case.description.clone(),
                    message: "parse answer is missing".to_string(),
                }),
                (Ok(_), Some(_)) => {}
                (Err(message), _) => failures.push(TestFailure {
                    filepath: filepath.to_path_buf(),
                    description: case.description.clone(),
                    message,
                }),
            }
            failures
        })
    }
}

fn parse_and_format<P>(parser: &P, input: &str) -> Result<Value, String>
where
    P: Parser,
{
    serde_json::to_value(parser.parse(input, None)).map_err(|error| error.to_string())
}
