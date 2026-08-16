use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const BLOCKQUOTE_TYPE: &str = "blockquote";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blockquote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}
