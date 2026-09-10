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
    // A string parse normally supplies one chunk. Reuse its character storage
    // instead of holding two full per-character buffers during line grouping.
    let mut chunk_ends = Vec::with_capacity(node_point_chunks.len());
    let mut chunks = node_point_chunks.into_iter();
    let mut all_node_points = if let Some(first) = chunks.next() {
        chunk_ends.push(first.len());
        first
    } else {
        Vec::new()
    };
    let remaining: usize = chunks.as_slice().iter().map(Vec::len).sum();
    all_node_points.reserve_exact(remaining);
    for mut chunk in chunks {
        all_node_points.append(&mut chunk);
        chunk_ends.push(all_node_points.len());
    }
    let mut line_groups: Vec<Vec<TempLine>> = Vec::new();

    let mut start_index = 0usize;
    let mut first_non_whitespace_index = 0usize;
    let mut count_of_precede_spaces = 0usize;

    let mut chunk_start = 0;
    for chunk_end in chunk_ends {
        let mut lines: Vec<TempLine> = Vec::new();

        for (offset, point) in all_node_points[chunk_start..chunk_end].iter().enumerate() {
            let index = chunk_start + offset;
            if first_non_whitespace_index == index && is_space_character(point.code_point) {
                count_of_precede_spaces += 1;
                first_non_whitespace_index += 1;
            }

            if is_line_ending(point.code_point) {
                if first_non_whitespace_index == index {
                    first_non_whitespace_index += 1;
                }

                lines.push(TempLine {
                    start_index,
                    end_index: index + 1,
                    first_non_whitespace_index,
                    indent_width: yozora_core_tokenizer::calc_indent_width(
                        &all_node_points,
                        start_index,
                        first_non_whitespace_index,
                    ),
                    count_of_precede_spaces,
                });

                start_index = index + 1;
                first_non_whitespace_index = start_index;
                count_of_precede_spaces = 0;
            }
        }

        line_groups.push(lines);
        chunk_start = chunk_end;
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

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::create_node_point_generator;

    #[test]
    fn retains_chunk_groups_and_partial_lines_across_empty_unicode_and_crlf_chunks() {
        let chunks = vec!["\tα\r", "\n", "", "β", "\nlast😀"];
        let groups = create_phrasing_line_groups(create_node_point_generator(chunks.clone()));
        assert_eq!(
            groups.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![0, 1, 0, 0, 1, 1]
        );
        let joined = chunks.concat();
        let whole = create_phrasing_line_groups(create_node_point_generator(joined.as_str()));
        let description = |line: &PhrasingContentLine| {
            (
                line.start_index,
                line.end_index,
                line.first_non_whitespace_index,
                line.indent_width,
                line.count_of_precede_spaces,
                line.node_points
                    .iter()
                    .map(|point| {
                        (
                            point.line,
                            point.column,
                            point.offset,
                            point.code_point,
                            point.source_width,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            groups.iter().flatten().map(description).collect::<Vec<_>>(),
            whole.iter().flatten().map(description).collect::<Vec<_>>()
        );
        let first = &groups[1][0];
        assert_eq!(first.first_non_whitespace_index, 4);
        assert_eq!(first.indent_width, 4);
        assert_eq!(first.count_of_precede_spaces, 4);
        assert_eq!(
            groups
                .iter()
                .flatten()
                .map(|line| {
                    (
                        line.start_index,
                        line.end_index,
                        line.first_non_whitespace_index,
                    )
                })
                .collect::<Vec<_>>(),
            vec![(0, 6, 4), (6, 8, 6), (8, 13, 8)]
        );
    }

    #[test]
    fn distinguishes_no_chunks_from_empty_chunks_and_retains_empty_lines() {
        assert!(create_phrasing_line_groups(Vec::new()).is_empty());
        assert_eq!(
            create_phrasing_line_groups(vec![Vec::new(), Vec::new()]).len(),
            2
        );
        let groups = create_phrasing_line_groups(create_node_point_generator("\n\n\n"));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);
        for line in &groups[0] {
            assert_eq!(line.first_non_whitespace_index, line.end_index);
            assert_eq!(line.indent_width, 0);
        }
    }
}
