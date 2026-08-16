use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const INLINE_CODE_TYPE: &str = "inlineCode";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineCode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}
