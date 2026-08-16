use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const THEMATIC_BREAK_TYPE: &str = "thematicBreak";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThematicBreak {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}
