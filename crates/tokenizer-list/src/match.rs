use yozora_ast::{NodeType, Position, TaskStatus, LIST_TYPE};
use yozora_character::{
    is_ascii_digit_character, is_ascii_lower_letter, is_ascii_upper_letter, is_space_character,
    is_whitespace_character, AsciiCodePoint, NodePoint, VirtualCodePoint,
};
use yozora_core_tokenizer::*;

use crate::types::ListTokenData;

#[derive(Debug, Clone)]
struct ParsedOpener {
    is_empty: bool,
    ordered: bool,
    marker: u32,
    order_type: Option<String>,
    order: Option<usize>,
    status: Option<TaskStatus>,
    indent: usize,
    count_of_top_blank_line: i32,
    next_index: usize,
}

#[derive(Debug, Clone)]
struct TaskStatusMatch {
    status: Option<TaskStatus>,
    next_index: usize,
}

pub(crate) fn eat_opener(
    line: &PhrasingContentLine,
    enable_task_list_item: bool,
) -> Option<EatOpenerResult> {
    let opener = parse_list_opener(line, enable_task_list_item)?;

    let token = BlockToken::new(
        "",
        LIST_TYPE,
        calc_segment_position(line, line.start_index, opener.next_index),
    )
    .with_data(ListTokenData {
        _is_empty: opener.is_empty,
        ordered: opener.ordered,
        marker: opener.marker,
        order_type: opener.order_type,
        order: opener.order,
        status: opener.status,
        indent: opener.indent,
        count_of_top_blank_line: opener.count_of_top_blank_line,
    });

    Some(EatOpenerResult {
        token,
        next_index: opener.next_index,
        saturated: false,
    })
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
    empty_item_could_not_interrupted_types: &[NodeType],
    enable_task_list_item: bool,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    let opener = eat_opener(line, enable_task_list_item)?;
    let data = opener.token.data_as::<ListTokenData>()?;

    if empty_item_could_not_interrupted_types.contains(&prev_sibling_token.node_type) {
        if data._is_empty {
            return None;
        }

        if data.ordered && data.order != Some(1) {
            return None;
        }
    }

    Some(EatAndInterruptPreviousSiblingResult {
        token: opener.token,
        next_index: opener.next_index,
        saturated: opener.saturated,
        remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    let Some(mut data) = token.data_as::<ListTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    let is_blank = line.first_non_whitespace_index >= line.end_index;
    if !is_blank && line.indent_width < data.indent {
        return EatContinuationTextResult::NotMatched;
    }

    let next_index = if is_blank {
        std::cmp::min(
            line.start_index + data.indent,
            line.end_index.saturating_sub(1),
        )
    } else {
        let Some(next_index) = eat_indentation(
            line.node_points.as_ref(),
            line.start_index,
            line.first_non_whitespace_index,
            data.indent,
        ) else {
            return EatContinuationTextResult::NotMatched;
        };
        next_index
    };

    if is_blank {
        if data.count_of_top_blank_line >= 0 {
            data.count_of_top_blank_line += 1;
            if data.count_of_top_blank_line > 1 {
                return EatContinuationTextResult::NotMatched;
            }
        }
    } else {
        data.count_of_top_blank_line = -1;
    }

    token.data = std::sync::Arc::new(data);
    update_token_end_position(token, line);

    EatContinuationTextResult::Opening { next_index }
}

fn parse_list_opener(
    line: &PhrasingContentLine,
    enable_task_list_item: bool,
) -> Option<ParsedOpener> {
    if line.indent_width >= 4 {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let end_index = line.end_index;
    let first_non_whitespace_index = line.first_non_whitespace_index;

    if first_non_whitespace_index >= end_index {
        return None;
    }

    let mut ordered = false;
    let mut marker: Option<i32> = None;
    let mut order_type: Option<&str> = None;
    let mut order: Option<usize> = None;

    let mut i = first_non_whitespace_index;
    let mut c = node_points[i].code_point;

    if i + 1 < end_index {
        let c0 = c;
        if is_ascii_digit_character(c0) {
            let mut value = (c0 - AsciiCodePoint::DIGIT0 as i32) as usize;
            i += 1;
            while i < end_index {
                c = node_points[i].code_point;
                if !is_ascii_digit_character(c) {
                    break;
                }

                value = value * 10 + (c - AsciiCodePoint::DIGIT0 as i32) as usize;
                i += 1;
            }
            order = Some(value);
            order_type = Some("1");
        } else if is_ascii_lower_letter(c0) {
            i += 1;
            c = node_points[i].code_point;
            order = Some((c0 - AsciiCodePoint::LOWERCASE_A as i32 + 1) as usize);
            order_type = Some("a");
        } else if is_ascii_upper_letter(c0) {
            i += 1;
            c = node_points[i].code_point;
            order = Some((c0 - AsciiCodePoint::UPPERCASE_A as i32 + 1) as usize);
            order_type = Some("A");
        }

        if i > first_non_whitespace_index
            && i - first_non_whitespace_index <= 9
            && (c == AsciiCodePoint::DOT as i32 || c == AsciiCodePoint::CLOSE_PARENTHESIS as i32)
        {
            i += 1;
            ordered = true;
            marker = Some(c);
        }
    }

    if !ordered {
        i = first_non_whitespace_index;
        c = node_points[i].code_point;
        if c == AsciiCodePoint::PLUS_SIGN as i32
            || c == AsciiCodePoint::MINUS_SIGN as i32
            || c == AsciiCodePoint::ASTERISK as i32
        {
            i += 1;
            marker = Some(c);
        }
    }

    let marker = marker? as u32;

    let marker_width = i - first_non_whitespace_index;
    let mut next_index = i;
    while next_index < end_index {
        c = node_points[next_index].code_point;
        if !is_space_character(c) {
            break;
        }
        next_index += 1;
    }
    let separator_end_index = next_index;
    let mut separator_width = calc_indent_width(node_points, i, separator_end_index);

    if separator_width > 4 {
        next_index = eat_indentation(node_points, i, separator_end_index, 1)?;
        separator_width = 1;
    }

    if separator_width == 0
        && next_index < end_index
        && node_points[next_index].code_point != VirtualCodePoint::LineEnd as i32
    {
        return None;
    }

    let mut count_of_top_blank_line = -1;
    if next_index < end_index
        && node_points[next_index].code_point == VirtualCodePoint::LineEnd as i32
    {
        count_of_top_blank_line = 1;

        if separator_width > 0 {
            next_index = eat_indentation(node_points, i, separator_end_index, 1)?;
        } else {
            next_index = separator_end_index + 1;
        }
        separator_width = 1;
    }

    let indent = line.indent_width + marker_width + separator_width;
    let is_empty = is_blank_range(node_points, next_index, end_index);

    let mut status = None;
    if enable_task_list_item {
        let result = eat_task_status(node_points, next_index, end_index);
        status = result.status;
        next_index = result.next_index;
    }

    Some(ParsedOpener {
        is_empty,
        ordered,
        marker,
        order_type: if ordered {
            order_type.map(str::to_string)
        } else {
            None
        },
        order: if ordered { order } else { None },
        status,
        indent,
        count_of_top_blank_line,
        next_index,
    })
}

fn eat_task_status(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> TaskStatusMatch {
    let mut i = start_index;
    while i < end_index {
        let c = node_points[i].code_point;
        if !is_space_character(c) {
            break;
        }

        i += 1;
    }

    if i + 3 >= end_index
        || node_points[i].code_point != AsciiCodePoint::OPEN_BRACKET as i32
        || node_points[i + 2].code_point != AsciiCodePoint::CLOSE_BRACKET as i32
        || !is_whitespace_character(node_points[i + 3].code_point)
    {
        return TaskStatusMatch {
            status: None,
            next_index: start_index,
        };
    }

    let status = match node_points[i + 1].code_point {
        x if x == AsciiCodePoint::SPACE as i32 => Some(TaskStatus::Todo),
        x if x == AsciiCodePoint::MINUS_SIGN as i32 => Some(TaskStatus::Doing),
        x if x == AsciiCodePoint::LOWERCASE_X as i32 || x == AsciiCodePoint::UPPERCASE_X as i32 => {
            Some(TaskStatus::Done)
        }
        _ => None,
    };

    if status.is_none() {
        return TaskStatusMatch {
            status: None,
            next_index: start_index,
        };
    }

    i += 3;
    while i < end_index && is_whitespace_character(node_points[i].code_point) {
        i += 1;
    }

    TaskStatusMatch {
        status,
        next_index: i,
    }
}

fn calc_segment_position(
    line: &PhrasingContentLine,
    start_index: usize,
    end_index: usize,
) -> Option<Position> {
    if start_index >= end_index {
        return None;
    }

    Some(Position {
        start: calc_start_point(line.node_points.as_ref(), start_index),
        end: calc_end_point(line.node_points.as_ref(), end_index - 1),
        indent: None,
    })
}

fn update_token_end_position(token: &mut BlockToken, line: &PhrasingContentLine) {
    let Some(position) = token.position.as_mut() else {
        return;
    };
    if line.start_index >= line.end_index {
        return;
    }

    position.end = calc_end_point(line.node_points.as_ref(), line.end_index - 1);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use yozora_character::create_node_point_generator;
    use yozora_core_tokenizer::PhrasingContentLine;

    use super::eat_opener;
    use crate::types::ListTokenData;

    #[test]
    fn task_marker_does_not_make_list_item_empty() {
        let node_points = create_node_point_generator("- [ ]\n")
            .pop()
            .expect("expected node points");
        let end_index = node_points.len();
        let line = PhrasingContentLine {
            node_points: Arc::new(node_points),
            start_index: 0,
            end_index,
            first_non_whitespace_index: 0,
            indent_width: 0,
            count_of_precede_spaces: 0,
        };

        let token = eat_opener(&line, true)
            .expect("expected task list token")
            .token;
        let data = token
            .data_as::<ListTokenData>()
            .expect("expected list token data");
        assert!(!data._is_empty);
    }
}
