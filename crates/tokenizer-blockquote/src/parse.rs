use yozora_ast::{Blockquote, Node, Text};

use crate::r#match::BlockquoteToken;

pub(crate) fn parse_blockquote_token(
    token: BlockquoteToken,
) -> yozora_core_tokenizer::BlockTokenizeResult {
    let children = if token.value.trim().is_empty() {
        Vec::new()
    } else {
        vec![Node::Text(Text {
            position: None,
            value: token.value,
        })]
    };

    yozora_core_tokenizer::BlockTokenizeResult {
        node: Node::Blockquote(Blockquote {
            position: None,
            children,
        }),
        consumed_lines: token.consumed_lines,
    }
}
