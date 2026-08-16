use std::sync::Arc;

use yozora_character::NodePoint;

pub const FOOTNOTE_DEFINITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-definition";

#[derive(Debug, Clone)]
pub struct FootnoteDefinitionLabel {
    pub node_points: Arc<Vec<NodePoint>>,
    pub start_index: usize,
    pub end_index: usize,
}

#[derive(Debug, Clone)]
pub struct FootnoteDefinitionTokenData {
    pub label: FootnoteDefinitionLabel,
    pub _label: Option<String>,
    pub _identifier: Option<String>,
}
