use yozora_ast::HEADING_TYPE;
use yozora_character::{is_space_like, AsciiCodePoint};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, EatAndInterruptPreviousSiblingResult,
    EatOpenerResult, PhrasingContentLine, RemainingSibling,
};

use crate::types::HeadingTokenData;

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width >= 4 {
        return None;
    }

    let first_non_whitespace_index = line.first_non_whitespace_index;
    if first_non_whitespace_index >= line.end_index
        || line.node_points[first_non_whitespace_index].code_point
            != AsciiCodePoint::NUMBER_SIGN as i32
    {
        return None;
    }

    let mut i = first_non_whitespace_index + 1;
    while i < line.end_index && line.node_points[i].code_point == AsciiCodePoint::NUMBER_SIGN as i32
    {
        i += 1;
    }

    let depth = i.saturating_sub(first_non_whitespace_index);
    if depth == 0 || depth > 6 {
        return None;
    }

    if i < line.end_index && !is_space_like(line.node_points[i].code_point) {
        return None;
    }

    let token =
        BlockToken::new("", HEADING_TYPE, calc_line_position(line)).with_data(HeadingTokenData {
            depth: depth as u8,
            line: line.clone(),
        });

    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: true,
    })
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    let opener = eat_opener(line)?;
    Some(EatAndInterruptPreviousSiblingResult {
        token: opener.token,
        next_index: opener.next_index,
        saturated: opener.saturated,
        remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
    })
}

fn calc_line_position(line: &PhrasingContentLine) -> Option<yozora_ast::Position> {
    if line.start_index >= line.end_index {
        return None;
    }

    Some(yozora_ast::Position {
        start: calc_start_point(line.node_points.as_ref(), line.start_index),
        end: calc_end_point(line.node_points.as_ref(), line.end_index - 1),
        indent: None,
    })
}
