use yozora_ast::{Node, Text};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub(crate) struct SoftBreakTokenData {
    pub value: String,
}

pub(crate) fn parse_soft_break_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<SoftBreakTokenData>() else {
            continue;
        };

        let position = if parse_api.shouldReservePosition() {
            Some(parse_api.calcPosition(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
        };

        nodes.push(Node::Text(Text {
            position,
            value: data.value.clone(),
        }));
    }

    nodes
}
