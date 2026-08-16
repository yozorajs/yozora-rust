use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const DEFINITION_TYPE: &str = "definition";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}
