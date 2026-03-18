use yozora_ast::{Code, Node};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::IndentedCodeToken;

pub(crate) fn parse_indented_code_token(token: IndentedCodeToken) -> BlockTokenizeResult {
    BlockTokenizeResult {
        node: Node::Code(Code {
            position: None,
            value: token.value,
            lang: None,
            meta: None,
        }),
        consumed_lines: token.consumed_lines,
    }
}
