use yozora_ast::{FootnoteDefinition, Node};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

#[derive(Debug, Clone)]
pub(crate) struct FootnoteDefinitionTokenData {
    pub label: String,
    pub identifier: String,
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

        let children = parse_api.parse_block_tokens(&token.children);
        nodes.push(Node::FootnoteDefinition(FootnoteDefinition {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            identifier: data.identifier.clone(),
            label: data.label.clone(),
            children,
        }));
    }

    nodes
}
