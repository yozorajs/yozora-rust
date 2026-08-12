use yozora_ast::{Emphasis, Node, Strong};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub struct EmphasisTokenData {
    pub thickness: usize,
}

pub(crate) fn parse_emphasis_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<EmphasisTokenData>() else {
            continue;
        };

        let children = parse_api.parse_inline_tokens(Some(&token.children));
        let position = if parse_api.should_reserve_position() {
            Some(parse_api.calc_position(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
        };

        if data.thickness == 2 {
            nodes.push(Node::Strong(Strong { position, children }));
        } else {
            nodes.push(Node::Emphasis(Emphasis { position, children }));
        }
    }

    nodes
}
