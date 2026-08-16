use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const TABLE_CELL_TYPE: &str = "tableCell";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}
