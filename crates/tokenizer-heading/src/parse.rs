use std::sync::Arc;

use yozora_ast::{Heading, Node};
use yozora_character::{
    calc_trim_boundary_of_code_points, is_whitespace_character, AsciiCodePoint,
};
use yozora_core_tokenizer::{
    merge_and_strip_content_lines, BlockToken, ParseBlockPhaseApi, PhrasingContentLine,
};

use crate::r#match::HeadingTokenData;

pub(crate) fn parse_heading_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<HeadingTokenData>() else {
            continue;
        };

        let line = &data.line;
        let node_points = line.node_points.as_ref();
        let (left_index, mut right_index) = calc_trim_boundary_of_code_points(
            node_points,
            line.first_non_whitespace_index + data.depth as usize,
            line.end_index,
        );

        let mut close_char_count = 0usize;
        let mut j = right_index;
        while j > left_index {
            let idx = j - 1;
            if node_points[idx].code_point != AsciiCodePoint::NUMBER_SIGN as i32 {
                break;
            }
            close_char_count += 1;
            j -= 1;
        }

        if close_char_count > 0 {
            let mut space_count = 0usize;
            let mut k = right_index - close_char_count;
            while k > left_index {
                let idx = k - 1;
                if !is_whitespace_character(node_points[idx].code_point) {
                    break;
                }
                space_count += 1;
                k -= 1;
            }

            if space_count > 0 || k == left_index {
                right_index -= close_char_count + space_count;
            }
        }

        let lines = vec![PhrasingContentLine {
            node_points: Arc::clone(&line.node_points),
            start_index: left_index,
            end_index: right_index,
            first_non_whitespace_index: left_index,
            count_of_precede_spaces: 0,
        }];

        let contents = merge_and_strip_content_lines(&lines, 0, lines.len());
        let children = parse_api.process_inlines(&contents);

        nodes.push(Node::Heading(Heading {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            identifier: None,
            depth: data.depth,
            children,
        }));
    }

    nodes
}
