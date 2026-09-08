use yozora_ast::{Footnote, Node};
use yozora_core_tokenizer::{
    parse_inline_containers, InlineToken, NodeInterval, ParseInlineHookResult, ParseInlinePhaseApi,
};

#[derive(Debug, Clone)]
pub(crate) struct FootnoteTokenData;

pub(crate) fn parse_footnote_tokens<'a>(
    tokens: &'a [InlineToken],
    parse_api: &'a dyn ParseInlinePhaseApi,
) -> ParseInlineHookResult<'a> {
    parse_inline_containers(
        tokens,
        move |token| {
            token.data_as::<FootnoteTokenData>()?;

            let position = if parse_api.should_reserve_position() {
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            Some(Footnote {
                position,
                children: Vec::new(),
            })
        },
        |mut node, children| {
            node.children = children;
            Node::Footnote(node)
        },
    )
}
