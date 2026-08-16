use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const IMAGE_TYPE: &str = "image";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub alt: String,
}
