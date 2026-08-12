use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseTesterProps {
    pub case_root_directory: PathBuf,
}

impl From<BaseTesterProps> for PathBuf {
    fn from(props: BaseTesterProps) -> Self {
        props.case_root_directory
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct YozoraUseCase<T = Value> {
    #[serde(default)]
    pub description: String,
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_answer: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markup_answer: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct YozoraUseCaseGroup<T = Value> {
    pub dirpath: PathBuf,
    pub filepath: PathBuf,
    pub title: Option<String>,
    pub cases: Vec<YozoraUseCase<T>>,
    pub sub_groups: Vec<YozoraUseCaseGroup<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestFailure {
    pub filepath: PathBuf,
    pub description: String,
    pub message: String,
}
