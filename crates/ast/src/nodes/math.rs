use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const MATH_TYPE: &str = "math";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Math {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}
