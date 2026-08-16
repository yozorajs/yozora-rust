use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const FOOTNOTE_REFERENCE_TYPE: &str = "footnoteReference";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootnoteReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
}
