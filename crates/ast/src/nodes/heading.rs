use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const HEADING_TYPE: &str = "heading";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heading {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    pub depth: u8,
    pub children: Vec<Node>,
}
