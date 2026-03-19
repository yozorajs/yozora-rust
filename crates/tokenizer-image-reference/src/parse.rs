use yozora_ast::{ImageReference, Node, ReferenceType};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};
use yozora_tokenizer_image::calc_image_alt;

#[derive(Debug, Clone)]
pub(crate) struct ImageReferenceTokenData {
    pub identifier: String,
    pub label: String,
    pub reference_type: ReferenceType,
    pub children_tokens: Vec<InlineToken>,
}

pub(crate) fn parse_image_reference_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<ImageReferenceTokenData>() else {
            continue;
        };

        let children = parse_api.parse_inline_tokens(&data.children_tokens);
        let alt = calc_image_alt(&children);

        let position = if parse_api.should_reserve_position() {
            parse_api.calc_position(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            })
        } else {
            None
        };

        nodes.push(Node::ImageReference(ImageReference {
            position,
            identifier: data.identifier.clone(),
            label: data.label.clone(),
            reference_type: data.reference_type,
            alt,
        }));
    }

    nodes
}
