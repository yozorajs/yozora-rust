use serde::{Deserialize, Serialize};

use crate::ast::{Position, ReferenceType};

pub const IMAGE_REFERENCE_TYPE: &str = "imageReference";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    #[serde(rename = "referenceType")]
    pub reference_type: ReferenceType,
    pub alt: String,
}
