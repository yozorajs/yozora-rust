use yozora_ast::{Node, Paragraph};
use yozora_core_tokenizer::{merge_and_strip_content_lines, BlockToken, ParseBlockPhaseApi};

use crate::r#match::ParagraphTokenData;

pub(crate) fn parse_paragraph_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<ParagraphTokenData>() else {
            continue;
        };

        let node_points = merge_and_strip_content_lines(&data.lines, 0, data.lines.len());
        let children = parse_api.process_inlines(&node_points);
        if children.is_empty() {
            continue;
        }

        let position = if parse_api.should_reserve_position() {
            token.position.clone()
        } else {
            None
        };

        nodes.push(Node::Paragraph(Paragraph { position, children }));
    }

    nodes
}
