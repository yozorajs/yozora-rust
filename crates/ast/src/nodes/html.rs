use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const HTML_TYPE: &str = "html";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HtmlContentType {
    Cdata,
    Closing,
    Comment,
    Declaration,
    Instruction,
    Open,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Html {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}
