use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const FRONTMATTER_TYPE: &str = "frontmatter";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,
}
