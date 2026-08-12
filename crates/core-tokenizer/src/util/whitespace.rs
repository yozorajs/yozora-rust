use yozora_character::{
    is_line_ending, is_space_character, is_whitespace_character, AsciiCodePoint, NodePoint,
    VirtualCodePoint,
};

use crate::PhrasingContentLine;

const TAB_SIZE: usize = 4;

pub fn eat_optional_characters(
    node_points: &[NodePoint],
    mut start_index: usize,
    end_index: usize,
    code_point: i32,
) -> usize {
    while start_index < end_index && node_points[start_index].code_point == code_point {
        start_index += 1;
    }
    start_index
}

pub fn eat_indentation(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    indent_width: usize,
) -> Option<usize> {
    if indent_width == 0 {
        return Some(start_index);
    }
    let direct_end_index = start_index + indent_width;
    if direct_end_index > end_index {
        return None;
    }
    if !node_points[start_index..direct_end_index]
        .iter()
        .any(|point| point.code_point == VirtualCodePoint::Space as i32)
    {
        return Some(direct_end_index);
    }

    let line = node_points[start_index].line;
    let mut line_start_index = start_index;
    while line_start_index > 0 && node_points[line_start_index - 1].line == line {
        line_start_index -= 1;
    }
    let mut column = 0usize;
    let mut remaining = indent_width;
    let mut index = line_start_index;
    while index < end_index {
        if index >= start_index && remaining == 0 {
            return Some(index);
        }
        let point = node_points[index];
        if point.code_point != VirtualCodePoint::Space as i32 {
            if index >= start_index {
                remaining = remaining.saturating_sub(1);
            }
            column += 1;
            index += 1;
            continue;
        }
        let mut tab_end_index = index + 1;
        while tab_end_index < end_index
            && node_points[tab_end_index].code_point == VirtualCodePoint::Space as i32
            && node_points[tab_end_index].offset == point.offset
        {
            tab_end_index += 1;
        }
        let tab_width = TAB_SIZE - column % TAB_SIZE;
        let padding = (tab_end_index - index).saturating_sub(tab_width);
        for tab_index in index..tab_end_index {
            let width = usize::from(tab_index - index >= padding);
            if tab_index >= start_index && width > 0 {
                if remaining == 0 {
                    return Some(tab_index);
                }
                remaining -= 1;
            }
            column += width;
        }
        index = tab_end_index;
    }
    (remaining == 0).then_some(end_index)
}

pub fn calc_indent_width(node_points: &[NodePoint], start_index: usize, end_index: usize) -> usize {
    if start_index >= end_index {
        return 0;
    }
    let contains_tab = node_points[start_index..end_index]
        .iter()
        .any(|point| point.code_point == VirtualCodePoint::Space as i32);
    if !contains_tab {
        return node_points[start_index..end_index]
            .iter()
            .take_while(|point| is_space_character(point.code_point))
            .count();
    }

    let line = node_points[start_index].line;
    let mut line_start_index = start_index;
    while line_start_index > 0 && node_points[line_start_index - 1].line == line {
        line_start_index -= 1;
    }
    let mut column = 0usize;
    let mut indent_width = 0usize;
    let mut index = line_start_index;
    while index < end_index {
        let point = node_points[index];
        if point.code_point != VirtualCodePoint::Space as i32 {
            if index >= start_index {
                if !is_space_character(point.code_point) {
                    break;
                }
                indent_width += 1;
            }
            column += 1;
            index += 1;
            continue;
        }
        let mut tab_end_index = index + 1;
        while tab_end_index < node_points.len()
            && node_points[tab_end_index].line == line
            && node_points[tab_end_index].code_point == VirtualCodePoint::Space as i32
            && node_points[tab_end_index].offset == point.offset
        {
            tab_end_index += 1;
        }
        let tab_width = TAB_SIZE - column % TAB_SIZE;
        let padding = (tab_end_index - index).saturating_sub(tab_width);
        for tab_index in index..tab_end_index.min(end_index) {
            let width = usize::from(tab_index - index >= padding);
            if tab_index >= start_index {
                indent_width += width;
            }
            column += width;
        }
        index = tab_end_index;
    }
    indent_width
}

pub fn eat_optional_whitespaces(
    node_points: &[NodePoint],
    mut start_index: usize,
    end_index: usize,
) -> usize {
    while start_index < end_index && is_whitespace_character(node_points[start_index].code_point) {
        start_index += 1;
    }
    start_index
}

pub fn eat_optional_whitespaces_reverse(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> usize {
    let mut index = end_index;
    while index > start_index && is_whitespace_character(node_points[index - 1].code_point) {
        index -= 1;
    }
    index
}

pub fn is_blank_range(node_points: &[NodePoint], start_index: usize, end_index: usize) -> bool {
    node_points[start_index..end_index].iter().all(|point| {
        point.code_point == AsciiCodePoint::SPACE as i32
            || point.code_point == VirtualCodePoint::Space as i32
            || point.code_point == VirtualCodePoint::LineEnd as i32
    })
}

pub fn eat_optional_blank_lines(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> usize {
    let mut result = start_index;
    for (index, point) in node_points
        .iter()
        .enumerate()
        .take(end_index)
        .skip(start_index)
    {
        if is_space_character(point.code_point) {
            continue;
        }
        if is_line_ending(point.code_point) {
            result = index + 1;
            continue;
        }
        break;
    }
    result
}

pub fn trim_blank_lines(lines: &[PhrasingContentLine]) -> Vec<PhrasingContentLine> {
    let Some(start) = lines
        .iter()
        .position(|line| line.first_non_whitespace_index < line.end_index)
    else {
        return Vec::new();
    };
    let end = lines
        .iter()
        .rposition(|line| line.first_non_whitespace_index < line.end_index)
        .unwrap_or(start);
    lines[start..=end].to_vec()
}

pub fn leading_indent_columns(line: &str) -> usize {
    split_indent_prefix(line, 0).0
}

pub fn split_indent_prefix(line: &str, base_column: usize) -> (usize, usize) {
    let mut column = base_column;
    let mut consumed_bytes = 0usize;
    for (index, character) in line.char_indices() {
        let width = match character {
            ' ' => 1,
            '\t' => TAB_SIZE - column % TAB_SIZE,
            _ => break,
        };
        column += width;
        consumed_bytes = index + character.len_utf8();
    }
    (column - base_column, consumed_bytes)
}

pub fn strip_indent_columns(line: &str, columns: usize) -> Option<String> {
    strip_indent_columns_with_base(line, columns, 0)
}

pub fn strip_indent_columns_with_base(
    line: &str,
    columns: usize,
    base_column: usize,
) -> Option<String> {
    if columns == 0 {
        return Some(line.to_string());
    }
    let mut column = base_column;
    let mut content_start = 0usize;
    for (index, character) in line.char_indices() {
        let width = match character {
            ' ' => 1,
            '\t' => TAB_SIZE - column % TAB_SIZE,
            _ => {
                content_start = index;
                break;
            }
        };
        column += width;
        content_start = index + character.len_utf8();
    }
    let total_indent = column - base_column;
    if total_indent < columns {
        return None;
    }
    Some(format!(
        "{}{}",
        " ".repeat(total_indent - columns),
        &line[content_start..]
    ))
}
