use yozora_ast::Position;
use yozora_character::{is_whitespace_character, NodePoint};

use crate::types::phrasing_content::PhrasingContentLine;
use crate::util::point::{calc_end_point, calc_start_point};

/// Calculate position from non-empty phrasing-content lines.
pub fn calc_position_from_phrasing_content_lines(lines: &[PhrasingContentLine]) -> Position {
    let first = &lines[0];
    let last = &lines[lines.len() - 1];

    Position {
        start: calc_start_point(first.node_points.as_ref(), first.start_index),
        end: calc_end_point(last.node_points.as_ref(), last.end_index - 1),
        indent: None,
    }
}

/// Merge line contents without trimming.
pub fn merge_content_lines_faithfully(
    lines: &[PhrasingContentLine],
    start_line_index: usize,
    end_line_index: usize,
) -> Vec<NodePoint> {
    if start_line_index >= end_line_index || end_line_index > lines.len() {
        return Vec::new();
    }

    let mut contents = Vec::new();
    for line in &lines[start_line_index..end_line_index] {
        if line.start_index >= line.end_index {
            continue;
        }

        contents.extend_from_slice(&line.node_points[line.start_index..line.end_index]);
    }

    contents
}

/// Merge line contents and strip leading spaces on each line,
/// and strip trailing spaces on the final line.
pub fn merge_and_strip_content_lines(
    lines: &[PhrasingContentLine],
    start_line_index: usize,
    end_line_index: usize,
) -> Vec<NodePoint> {
    if start_line_index >= end_line_index || end_line_index > lines.len() {
        return Vec::new();
    }

    let mut contents = Vec::new();

    for line in &lines[start_line_index..(end_line_index - 1)] {
        if line.first_non_whitespace_index >= line.end_index {
            continue;
        }

        contents
            .extend_from_slice(&line.node_points[line.first_non_whitespace_index..line.end_index]);
    }

    let last = &lines[end_line_index - 1];
    if last.first_non_whitespace_index >= last.end_index {
        return contents;
    }

    let node_points = last.node_points.as_ref();
    let mut right = last.end_index;
    while right > last.first_non_whitespace_index {
        let idx = right - 1;
        if !is_whitespace_character(node_points[idx].code_point) {
            break;
        }
        right -= 1;
    }

    if right > last.first_non_whitespace_index {
        contents.extend_from_slice(&node_points[last.first_non_whitespace_index..right]);
    }

    contents
}
