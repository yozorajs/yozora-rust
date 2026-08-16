use serde::{Deserialize, Serialize};

use crate::ast::Position;

use super::{take_node_children, Node};

pub const ROOT_TYPE: &str = "root";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Root {
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

impl Default for Root {
    fn default() -> Self {
        Self {
            node_type: ROOT_TYPE.to_string(),
            position: None,
            children: Vec::new(),
        }
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.children);
        while let Some(mut node) = stack.pop() {
            take_node_children(&mut node, &mut stack);
        }
    }
}
