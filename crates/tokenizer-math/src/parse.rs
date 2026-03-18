use yozora_ast::{Math, Node};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::MathToken;

pub(crate) fn parse_math_token(token: MathToken) -> BlockTokenizeResult {
    BlockTokenizeResult {
        node: Node::Math(Math {
            position: None,
            value: token.value,
        }),
        consumed_lines: token.consumed_lines,
    }
}
