use std::sync::Arc;

use yozora_character::{is_line_ending, is_space_character, NodePoint};
use yozora_core_tokenizer::PhrasingContentLine;

#[derive(Debug, Clone, Copy)]
struct TempLine {
    start_index: usize,
    end_index: usize,
    first_non_whitespace_index: usize,
    indent_width: usize,
    count_of_precede_spaces: usize,
}

pub fn create_phrasing_line_generator(
    node_point_chunks: Vec<Vec<NodePoint>>,
) -> impl Iterator<Item = Vec<PhrasingContentLine>> {
    let mut all_node_points: Vec<NodePoint> = Vec::new();
    let mut line_groups: Vec<Vec<TempLine>> = Vec::new();

    let mut start_index = 0usize;
    let mut first_non_whitespace_index = 0usize;
    let mut count_of_precede_spaces = 0usize;

    for chunk in node_point_chunks {
        let mut lines: Vec<TempLine> = Vec::new();

        for point in chunk {
            if first_non_whitespace_index == all_node_points.len()
                && is_space_character(point.code_point)
            {
                count_of_precede_spaces += 1;
                first_non_whitespace_index += 1;
            }

            all_node_points.push(point);
            if is_line_ending(point.code_point) {
                if first_non_whitespace_index + 1 == all_node_points.len() {
                    first_non_whitespace_index += 1;
                }

                lines.push(TempLine {
                    start_index,
                    end_index: all_node_points.len(),
                    first_non_whitespace_index,
                    indent_width: yozora_core_tokenizer::calc_indent_width(
                        &all_node_points,
                        start_index,
                        first_non_whitespace_index,
                    ),
                    count_of_precede_spaces,
                });

                start_index = all_node_points.len();
                first_non_whitespace_index = all_node_points.len();
                count_of_precede_spaces = 0;
            }
        }

        line_groups.push(lines);
    }

    if start_index < all_node_points.len() {
        line_groups.push(vec![TempLine {
            start_index,
            end_index: all_node_points.len(),
            first_non_whitespace_index,
            indent_width: yozora_core_tokenizer::calc_indent_width(
                &all_node_points,
                start_index,
                first_non_whitespace_index,
            ),
            count_of_precede_spaces,
        }]);
    }

    let shared_points = Arc::new(all_node_points);
    line_groups.into_iter().map(move |group| {
        group
            .into_iter()
            .map(|line| PhrasingContentLine {
                node_points: shared_points.clone(),
                start_index: line.start_index,
                end_index: line.end_index,
                first_non_whitespace_index: line.first_non_whitespace_index,
                indent_width: line.indent_width,
                count_of_precede_spaces: line.count_of_precede_spaces,
            })
            .collect()
    })
}

pub fn create_phrasing_line_groups(
    node_point_chunks: Vec<Vec<NodePoint>>,
) -> Vec<Vec<PhrasingContentLine>> {
    create_phrasing_line_generator(node_point_chunks).collect()
}
