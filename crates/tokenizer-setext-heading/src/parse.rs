use yozora_ast::{Heading, Node, Text};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::SetextHeadingToken;

pub(crate) fn parse_setext_heading_token(token: SetextHeadingToken) -> BlockTokenizeResult {
    BlockTokenizeResult {
        node: Node::Heading(Heading {
            position: None,
            identifier: None,
            depth: token.depth,
            children: vec![Node::Text(Text {
                position: None,
                value: token.content,
            })],
        }),
        consumed_lines: token.consumed_lines,
    }
}
