use yozora_ast::{drop_nodes, ImageReference, Node};
use yozora_core_tokenizer::{
    parse_inline_containers, InlineToken, NodeInterval, ParseInlineHookResult, ParseInlinePhaseApi,
};
use yozora_tokenizer_image::calc_image_alt;

use crate::types::ImageReferenceTokenData;

pub(crate) fn parse_image_reference_tokens<'a>(
    tokens: &'a [InlineToken],
    parse_api: &'a dyn ParseInlinePhaseApi,
) -> ParseInlineHookResult<'a> {
    parse_inline_containers(
        tokens,
        |token| Some((token, token.data_as::<ImageReferenceTokenData>()?)),
        move |(token, data), children| {
            let alt = calc_image_alt(&children);
            drop_nodes(children);

            let position = if parse_api.should_reserve_position() {
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            Node::ImageReference(ImageReference {
                position,
                identifier: data.identifier.clone(),
                label: data.label.clone(),
                reference_type: data.reference_type,
                alt,
            })
        },
    )
}
