use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const FOOTNOTE_TYPE: &str = "footnote";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Footnote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}
