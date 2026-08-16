use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const ADMONITION_TYPE: &str = "admonition";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Admonition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub keyword: String,
    pub title: Vec<Node>,
    pub children: Vec<Node>,
}
