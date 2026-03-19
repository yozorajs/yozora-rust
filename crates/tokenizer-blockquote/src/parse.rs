use yozora_ast::{Blockquote, Node};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

pub(crate) fn parse_blockquote_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let children = parse_api.parseBlockTokens(Some(&token.children));
        let position = if parse_api.shouldReservePosition() {
            token.position.clone()
        } else {
            None
        };
        nodes.push(Node::Blockquote(Blockquote { position, children }));
    }

    nodes
}
