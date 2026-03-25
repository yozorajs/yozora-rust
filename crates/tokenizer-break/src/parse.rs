use yozora_ast::{BreakNode, Node};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

pub(crate) fn parse_break_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let position = if parse_api.shouldReservePosition() {
            Some(parse_api.calcPosition(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
        };

        nodes.push(Node::Break(BreakNode { position }));
    }

    nodes
}
