use yozora_ast::{Code, Node};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::FencedCodeToken;

pub(crate) fn parse_fenced_code_token(token: FencedCodeToken) -> BlockTokenizeResult {
    BlockTokenizeResult {
        node: Node::Code(Code {
            position: None,
            value: token.value,
            lang: token.lang,
            meta: token.meta,
        }),
        consumed_lines: token.consumed_lines,
    }
}
