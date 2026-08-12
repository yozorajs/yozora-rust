use yozora_ast::{LinkReference, Node, ReferenceType, Text};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub struct LinkReferenceTokenData {
    pub identifier: String,
    pub label: String,
    pub reference_type: ReferenceType,
    pub(crate) child_text: String,
}

pub(crate) fn parse_link_reference_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<LinkReferenceTokenData>() else {
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

        nodes.push(Node::LinkReference(LinkReference {
            position,
            identifier: data.identifier.clone(),
            label: data.label.clone(),
            reference_type: data.reference_type,
            children: if token.children.is_empty() {
                vec![Node::Text(Text {
                    position: None,
                    value: data.child_text.clone(),
                })]
            } else {
                parse_api.parse_inline_tokens(Some(&token.children))
            },
        }));
    }

    nodes
}
