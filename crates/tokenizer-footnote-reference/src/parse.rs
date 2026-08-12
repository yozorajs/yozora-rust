use yozora_ast::{FootnoteReference, Node};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub(crate) struct FootnoteReferenceTokenData {
    pub identifier: String,
    pub label: String,
}

pub(crate) fn parse_footnote_reference_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<FootnoteReferenceTokenData>() else {
            continue;
        };

        let position = if parse_api.should_reserve_position() {
            Some(parse_api.calc_position(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
        };

        nodes.push(Node::FootnoteReference(FootnoteReference {
            position,
            identifier: data.identifier.clone(),
            label: data.label.clone(),
        }));
    }

    nodes
}
