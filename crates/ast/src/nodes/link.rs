use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const LINK_TYPE: &str = "link";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub children: Vec<Node>,
}
