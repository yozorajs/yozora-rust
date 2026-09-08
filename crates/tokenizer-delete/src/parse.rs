use yozora_ast::{DeleteNode, Node};
use yozora_core_tokenizer::{
    parse_inline_containers, InlineToken, NodeInterval, ParseInlineHookResult, ParseInlinePhaseApi,
};

use crate::types::DeleteTokenData;

pub(crate) fn parse_delete_tokens<'a>(
    tokens: &'a [InlineToken],
    parse_api: &'a dyn ParseInlinePhaseApi,
) -> ParseInlineHookResult<'a> {
    parse_inline_containers(
        tokens,
        |token| {
            token.data_as::<DeleteTokenData>()?;
            Some(token)
        },
        move |token, children| {
            let position = if parse_api.should_reserve_position() {
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            Node::Delete(DeleteNode { position, children })
        },
    )
}
