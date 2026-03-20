use yozora_ast::{NodeType, Point, Position, TaskStatus, LIST_TYPE};
use yozora_character::{
    is_ascii_digit_character, is_ascii_lower_letter, is_ascii_upper_letter, is_space_character,
    is_whitespace_character, AsciiCodePoint, NodePoint, VirtualCodePoint,
};
use yozora_core_tokenizer::*;

#[derive(Debug, Clone)]
pub(crate) struct TokenData {
    pub ordered: bool,
    pub marker: u32,
    pub order_type: Option<String>,
    pub order: Option<usize>,
    pub status: Option<TaskStatus>,
    pub indent: usize,
    pub count_of_top_blank_line: i32,
}

#[derive(Debug, Clone)]
struct ParsedOpener {
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
    .with_data(TokenData {
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
    let data = opener.token.data_as::<TokenData>()?;

    if empty_item_could_not_interrupted_types.contains(&prev_sibling_token.node_type) {
        if data.indent == line.end_index.saturating_sub(line.start_index) {
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
    let Some(mut data) = token.data_as::<TokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    if line.first_non_whitespace_index < line.end_index
        && line.count_of_precede_spaces < data.indent
    {
        return EatContinuationTextResult::NotMatched;
    }

    if line.first_non_whitespace_index >= line.end_index {
        if data.count_of_top_blank_line >= 0 {
            data.count_of_top_blank_line += 1;
            if data.count_of_top_blank_line > 1 {
                return EatContinuationTextResult::NotMatched;
            }
        }
    } else {
        data.count_of_top_blank_line = -1;
    }

    let indent = data.indent;
    token.data = std::sync::Arc::new(data);
    update_token_end_position(token, line);

    let next_index = std::cmp::min(line.start_index + indent, line.end_index.saturating_sub(1));

    EatContinuationTextResult::Opening { next_index }
}

fn parse_list_opener(
    line: &PhrasingContentLine,
    enable_task_list_item: bool,
) -> Option<ParsedOpener> {
    if line.count_of_precede_spaces >= 4 {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let start_index = line.start_index;
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

    let mut count_of_spaces = 0usize;
    let mut next_index = i;
    if next_index < end_index
        && node_points[next_index].code_point == VirtualCodePoint::Space as i32
    {
        next_index += 1;
    }

    while next_index < end_index {
        c = node_points[next_index].code_point;
        if !is_space_character(c) {
            break;
        }

        count_of_spaces += 1;
        next_index += 1;
    }

    if count_of_spaces > 4 {
        next_index -= count_of_spaces - 1;
        count_of_spaces = 1;
    }

    if count_of_spaces == 0
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

        // Keep behavior aligned with TS implementation:
        // nextIndex -= countOfSpaces - 1
        // which means +1 when count_of_spaces == 0.
        if count_of_spaces == 0 {
            next_index += 1;
        } else {
            next_index -= count_of_spaces - 1;
        }
        count_of_spaces = 1;
    }

    let indent = i - start_index + count_of_spaces;

    let mut status = None;
    if enable_task_list_item {
        let result = eat_task_status(node_points, next_index, end_index);
        status = result.status;
        next_index = result.next_index;
    }

    Some(ParsedOpener {
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

    TaskStatusMatch {
        status,
        next_index: i + 4,
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

    let start = line.node_points[start_index];
    let end = line.node_points[end_index - 1];

    Some(Position {
        start: Point {
            line: start.line,
            column: start.column,
            offset: Some(start.offset),
        },
        end: Point {
            line: end.line,
            column: end.column + 1,
            offset: Some(end.offset + 1),
        },
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

    let end = line.node_points[line.end_index - 1];
    position.end = Point {
        line: end.line,
        column: end.column + 1,
        offset: Some(end.offset + 1),
    };
}
