use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const EMPHASIS_TYPE: &str = "emphasis";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Emphasis {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}
