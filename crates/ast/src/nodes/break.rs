use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const BREAK_TYPE: &str = "break";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Break {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

pub type BreakNode = Break;
