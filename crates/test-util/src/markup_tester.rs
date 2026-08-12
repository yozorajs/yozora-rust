use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use yozora_ast::Root;
use yozora_ast_util::remove_positions;
use yozora_core_parser::{ParseOptions, Parser};
use yozora_markup_weaver::MarkupWeaverContract;

use crate::{BaseTester, TestFailure, YozoraUseCaseGroup};

pub struct MarkupTester<P, W> {
    base: BaseTester,
    pub parser: P,
    pub weaver: W,
}

impl<P, W> MarkupTester<P, W>
where
    P: Parser,
    W: MarkupWeaverContract,
{
    pub fn new(case_root_directory: impl Into<PathBuf>, parser: P, weaver: W) -> Self {
        Self {
            base: BaseTester::new(case_root_directory),
            parser,
            weaver,
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

    pub fn weave(&self, input: &str, options: Option<ParseOptions>) -> String {
        let ast = self.parser.parse(input, options);
        self.weaver.weave(&ast)
    }

    pub fn run_answer(&mut self) -> Result<(), String> {
        let root = self.base.case_root_directory().to_path_buf();
        let parser = &self.parser;
        let weaver = &self.weaver;
        for group in self.base.case_groups_mut() {
            answer_group(parser, weaver, &root, group)?;
        }
        Ok(())
    }

    pub fn run_test(&self) -> Result<(), Vec<TestFailure>> {
        let mut failures = Vec::new();
        for group in self.base.collect() {
            test_group(&self.parser, &self.weaver, &group, &mut failures);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }
}

struct WeaveResult {
    markup: String,
    expected_ast: Value,
    received_ast: Value,
}

fn weave_and_format<P, W>(parser: &P, weaver: &W, input: &str) -> Result<WeaveResult, String>
where
    P: Parser,
    W: MarkupWeaverContract,
{
    let options = Some(ParseOptions {
        should_reserve_position: Some(true),
        ..ParseOptions::default()
    });
    let expected_ast = parser.parse(input, options.clone());
    let markup = weaver.weave(&expected_ast);
    let received_ast = parser.parse(&markup, options);
    Ok(WeaveResult {
        markup,
        expected_ast: normalize_ast(&expected_ast)?,
        received_ast: normalize_ast(&received_ast)?,
    })
}

fn normalize_ast(ast: &Root) -> Result<Value, String> {
    serde_json::to_value(remove_positions(ast)).map_err(|error| error.to_string())
}

fn are_same_ast(expected: &Value, received: &Value) -> bool {
    let mut expected = expected.clone();
    let mut received = received.clone();
    remove_reference_types(&mut expected);
    remove_reference_types(&mut received);
    expected == received
}

fn remove_reference_types(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                remove_reference_types(value);
            }
        }
        Value::Object(object) => {
            let is_reference =
                object
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|node_type| {
                        node_type == "linkReference" || node_type == "imageReference"
                    });
            if is_reference {
                object.remove("referenceType");
            }
            for value in object.values_mut() {
                remove_reference_types(value);
            }
        }
        _ => {}
    }
}

fn answer_group<P, W>(
    parser: &P,
    weaver: &W,
    parent_dir: &Path,
    group: &mut YozoraUseCaseGroup,
) -> Result<(), String>
where
    P: Parser,
    W: MarkupWeaverContract,
{
    if group.dirpath == group.filepath {
        for subgroup in &mut group.sub_groups {
            answer_group(parser, weaver, &group.dirpath, subgroup)?;
        }
        return Ok(());
    }

    for case in &mut group.cases {
        case.markup_answer = Some(
            weave_and_format(parser, weaver, &case.input)
                .map_err(|error| format!("[handle failed] {}: {error}", group.filepath.display()))?
                .markup,
        );
    }
    write_group(parent_dir, group)
}

fn test_group<P, W>(
    parser: &P,
    weaver: &W,
    group: &YozoraUseCaseGroup,
    failures: &mut Vec<TestFailure>,
) where
    P: Parser,
    W: MarkupWeaverContract,
{
    for case in &group.cases {
        match weave_and_format(parser, weaver, &case.input) {
            Ok(result) => {
                if case.markup_answer.as_ref() != Some(&result.markup) {
                    failures.push(TestFailure {
                        filepath: group.filepath.clone(),
                        description: case.description.clone(),
                        message: format!(
                            "markup answer mismatch\nexpected={:?}\nactual={:?}",
                            case.markup_answer, result.markup
                        ),
                    });
                }
                if !are_same_ast(&result.expected_ast, &result.received_ast) {
                    failures.push(TestFailure {
                        filepath: group.filepath.clone(),
                        description: case.description.clone(),
                        message: format!(
                            "roundtrip AST mismatch\nexpected={}\nactual={}",
                            result.expected_ast, result.received_ast
                        ),
                    });
                }
            }
            Err(message) => failures.push(TestFailure {
                filepath: group.filepath.clone(),
                description: case.description.clone(),
                message,
            }),
        }
    }
    for subgroup in &group.sub_groups {
        test_group(parser, weaver, subgroup, failures);
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
