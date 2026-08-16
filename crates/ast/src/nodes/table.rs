use serde::{Deserialize, Serialize};

use crate::ast::{AlignType, Position};

use super::Node;

pub const TABLE_TYPE: &str = "table";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableColumn {
    pub align: Option<AlignType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub columns: Vec<TableColumn>,
    pub children: Vec<Node>,
}
