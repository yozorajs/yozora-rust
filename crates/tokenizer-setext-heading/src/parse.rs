use yozora_ast::{Heading, Node, NodeBuffer};
use yozora_character::AsciiCodePoint;
use yozora_core_tokenizer::{merge_and_strip_content_lines, BlockToken, ParseBlockPhaseApi};

use crate::types::SetextHeadingTokenData;

pub(crate) fn parse_setext_heading_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = NodeBuffer::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<SetextHeadingTokenData>() else {
            continue;
        };

        let depth: u8 = match data.marker {
            marker if marker == AsciiCodePoint::EQUALS_SIGN as i32 => 1,
            marker if marker == AsciiCodePoint::MINUS_SIGN as i32 => 2,
            _ => 1,
        };

        let contents = merge_and_strip_content_lines(&data.lines, 0, data.lines.len());
        let children = parse_api.process_inlines(&contents);

        nodes.push(Node::Heading(Heading {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            identifier: None,
            depth,
            children,
        }));
    }

    nodes.into_vec()
}
