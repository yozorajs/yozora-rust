use yozora_ast::{LinkReference, Node};
use yozora_core_tokenizer::{
    parse_inline_containers, InlineToken, NodeInterval, ParseInlineHookResult, ParseInlinePhaseApi,
};

use crate::types::LinkReferenceTokenData;

pub(crate) fn parse_link_reference_tokens<'a>(
    tokens: &'a [InlineToken],
    parse_api: &'a dyn ParseInlinePhaseApi,
) -> ParseInlineHookResult<'a> {
    parse_inline_containers(
        tokens,
        move |token| {
            let data = token.data_as::<LinkReferenceTokenData>()?;

            let position = if parse_api.should_reserve_position() {
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            Some(LinkReference {
                position,
                identifier: data.identifier.clone(),
                label: data.label.clone(),
                reference_type: data.reference_type,
                children: Vec::new(),
            })
        },
        |mut node, children| {
            node.children = children;
            Node::LinkReference(node)
        },
    )
}
