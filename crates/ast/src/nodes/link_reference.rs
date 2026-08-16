use serde::{Deserialize, Serialize};

use crate::ast::{Position, ReferenceType};

use super::Node;

pub const LINK_REFERENCE_TYPE: &str = "linkReference";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    #[serde(rename = "referenceType")]
    pub reference_type: ReferenceType,
    pub children: Vec<Node>,
}
