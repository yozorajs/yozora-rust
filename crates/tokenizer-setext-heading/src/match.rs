use yozora_ast::HEADING_TYPE;
use yozora_character::{is_whitespace_character, AsciiCodePoint, VirtualCodePoint};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, EatAndInterruptPreviousSiblingResult,
    PhrasingContentLine, RemainingSibling,
};

#[derive(Debug, Clone)]
pub struct SetextHeadingTokenData {
    pub marker: i32,
    pub lines: Vec<PhrasingContentLine>,
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    _prev_sibling_token: &BlockToken,
    phrasing_lines: Option<Vec<PhrasingContentLine>>,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    if line.count_of_precede_spaces >= 4 || line.first_non_whitespace_index >= line.end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let mut marker: Option<i32> = None;
    let mut has_potential_internal_space = false;

    for point in node_points
        .iter()
        .take(line.end_index)
        .skip(line.first_non_whitespace_index)
    {
        let code_point = point.code_point;
        if code_point == VirtualCodePoint::LineEnd as i32 {
            break;
        }

        if is_whitespace_character(code_point) {
            has_potential_internal_space = true;
            continue;
        }

        if has_potential_internal_space
            || (code_point != AsciiCodePoint::EQUALS_SIGN as i32
                && code_point != AsciiCodePoint::MINUS_SIGN as i32)
            || marker.is_some_and(|m| m != code_point)
        {
            marker = None;
            break;
        }

        marker = Some(code_point);
    }

    let marker = marker?;
    let lines = phrasing_lines?;
    let first_line = lines.first()?;

    let token = BlockToken::new("", HEADING_TYPE, calc_spanning_position(first_line, line))
        .with_data(SetextHeadingTokenData { marker, lines });

    Some(EatAndInterruptPreviousSiblingResult {
        token,
        next_index: line.end_index,
        saturated: true,
        remaining_sibling: RemainingSibling::None,
    })
}

fn calc_spanning_position(
    first: &PhrasingContentLine,
    last: &PhrasingContentLine,
) -> Option<yozora_ast::Position> {
    if first.start_index >= first.end_index || last.start_index >= last.end_index {
        return None;
    }

    Some(yozora_ast::Position {
        start: calc_start_point(first.node_points.as_ref(), first.start_index),
        end: calc_end_point(last.node_points.as_ref(), last.end_index - 1),
        indent: None,
    })
}
