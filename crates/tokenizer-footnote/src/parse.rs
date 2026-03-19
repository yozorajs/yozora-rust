use yozora_ast::{Footnote, Node};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub(crate) struct FootnoteTokenData {
    pub children_tokens: Vec<InlineToken>,
}

pub(crate) fn parse_footnote_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<FootnoteTokenData>() else {
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

        nodes.push(Node::Footnote(Footnote {
            position,
            children: parse_api.parseInlineTokens(Some(&data.children_tokens)),
        }));
    }

    nodes
}
