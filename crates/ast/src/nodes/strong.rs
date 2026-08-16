use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const STRONG_TYPE: &str = "strong";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strong {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}
