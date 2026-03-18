use yozora_ast::{FootnoteDefinition, Node, Text};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::FootnoteDefinitionBlockToken;

pub(crate) fn parse_footnote_definition_token(
    token: FootnoteDefinitionBlockToken,
) -> BlockTokenizeResult {
    BlockTokenizeResult {
        node: Node::FootnoteDefinition(FootnoteDefinition {
            position: None,
            identifier: token.identifier,
            label: token.label,
            children: vec![Node::Text(Text {
                position: None,
                value: token.value,
            })],
        }),
        consumed_lines: token.consumed_lines,
    }
}
