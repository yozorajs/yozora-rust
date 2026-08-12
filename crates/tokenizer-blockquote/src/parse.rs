use yozora_ast::{Blockquote, Node};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

pub(crate) fn parse_blockquote_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let children = parse_api.parse_block_tokens(Some(&token.children));
        let position = if parse_api.should_reserve_position() {
            token.position.clone()
        } else {
            None
        };
        nodes.push(Node::Blockquote(Blockquote { position, children }));
    }

    nodes
}
