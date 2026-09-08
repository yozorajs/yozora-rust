use yozora_ast::{Emphasis, Node, Strong};
use yozora_core_tokenizer::{
    parse_inline_containers, InlineToken, NodeInterval, ParseInlineHookResult, ParseInlinePhaseApi,
};

use crate::types::EmphasisTokenData;

pub(crate) fn parse_emphasis_tokens<'a>(
    tokens: &'a [InlineToken],
    parse_api: &'a dyn ParseInlinePhaseApi,
) -> ParseInlineHookResult<'a> {
    parse_inline_containers(
        tokens,
        |token| Some((token, token.data_as::<EmphasisTokenData>()?)),
        move |(token, data), children| {
            let position = if parse_api.should_reserve_position() {
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            if data.thickness == 2 {
                Node::Strong(Strong { position, children })
            } else {
                Node::Emphasis(Emphasis { position, children })
            }
        },
    )
}
