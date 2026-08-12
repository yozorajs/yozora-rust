use std::sync::Arc;

use yozora_ast::{FootnoteDefinition, Node};
use yozora_character::NodePoint;
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

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

pub(crate) fn parse_footnote_definition_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<FootnoteDefinitionTokenData>() else {
            continue;
        };

        let children = parse_api.parse_block_tokens(Some(&token.children));
        nodes.push(Node::FootnoteDefinition(FootnoteDefinition {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            identifier: data._identifier.clone().unwrap_or_default(),
            label: data._label.clone().unwrap_or_default(),
            children,
        }));
    }

    nodes
}
