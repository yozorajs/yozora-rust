use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const FOOTNOTE_DEFINITION_TYPE: &str = "footnoteDefinition";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootnoteDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    pub children: Vec<Node>,
}
