use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const TABLE_ROW_TYPE: &str = "tableRow";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}
