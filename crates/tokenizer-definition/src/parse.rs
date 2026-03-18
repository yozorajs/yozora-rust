use yozora_ast::{Definition, Node};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::DefinitionBlockToken;

pub(crate) fn parse_definition_token(token: DefinitionBlockToken) -> BlockTokenizeResult {
    BlockTokenizeResult {
        node: Node::Definition(Definition {
            position: None,
            identifier: token.identifier,
            label: token.label,
            url: token.url,
            title: token.title,
        }),
        consumed_lines: token.consumed_lines,
    }
}
