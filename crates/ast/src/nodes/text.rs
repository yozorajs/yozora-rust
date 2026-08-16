use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const TEXT_TYPE: &str = "text";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}
