use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const LIST_TYPE: &str = "list";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub ordered: bool,
    #[serde(rename = "orderType", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    pub marker: u32,
    pub spread: bool,
    pub children: Vec<Node>,
}
