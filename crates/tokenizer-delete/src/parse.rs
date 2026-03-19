use yozora_ast::{DeleteNode, Node};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub(crate) struct DeleteTokenData {
    pub children: Vec<InlineToken>,
}

pub(crate) fn parse_delete_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<DeleteTokenData>() else {
            continue;
        };

        let children = parse_api.parseInlineTokens(Some(&data.children));
        let position = if parse_api.shouldReservePosition() {
            Some(parse_api.calcPosition(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
        };

        nodes.push(Node::Delete(DeleteNode { position, children }));
    }

    nodes
}
