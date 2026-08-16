use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const PARAGRAPH_TYPE: &str = "paragraph";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}
