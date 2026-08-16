use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::Node;

pub const DELETE_TYPE: &str = "delete";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

pub type DeleteNode = Delete;
