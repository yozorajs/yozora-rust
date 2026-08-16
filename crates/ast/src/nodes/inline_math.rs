use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const INLINE_MATH_TYPE: &str = "inlineMath";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineMath {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}
