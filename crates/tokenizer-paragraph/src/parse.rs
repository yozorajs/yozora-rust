use yozora_ast::{Node, NodeBuffer, Paragraph};
use yozora_character::calc_trim_boundary_of_code_points;
use yozora_core_tokenizer::{merge_and_strip_content_lines, BlockToken, ParseBlockPhaseApi};

use crate::types::ParagraphTokenData;

pub(crate) fn parse_paragraph_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = NodeBuffer::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<ParagraphTokenData>() else {
            continue;
        };

        let children = if data.lines.len() == 1 {
            let line = &data.lines[0];
            let (_, content_end_index) = calc_trim_boundary_of_code_points(
                line.node_points.as_ref(),
                line.first_non_whitespace_index,
                line.end_index,
            );
            parse_api.process_inlines(
                &line.node_points[line.first_non_whitespace_index..content_end_index],
            )
        } else {
            let node_points = merge_and_strip_content_lines(&data.lines, 0, data.lines.len());
            parse_api.process_inlines(&node_points)
        };
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

    nodes.into_vec()
}
