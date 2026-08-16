use yozora_ast::THEMATIC_BREAK_TYPE;
use yozora_character::{is_whitespace_character, AsciiCodePoint};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, EatAndInterruptPreviousSiblingResult,
    EatOpenerResult, PhrasingContentLine, RemainingSibling,
};

use crate::types::ThematicBreakTokenData;

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width >= 4 {
        return None;
    }

    if line.first_non_whitespace_index + 2 >= line.end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let mut marker: Option<i32> = None;
    let mut count = 0usize;
    let mut continuous = true;
    let mut has_potential_internal_space = false;

    for point in node_points
        .iter()
        .take(line.end_index)
        .skip(line.first_non_whitespace_index)
    {
        let code_point = point.code_point;
        if is_whitespace_character(code_point) {
            has_potential_internal_space = true;
            continue;
        }

        if has_potential_internal_space {
            continuous = false;
        }

        match code_point {
            x if x == AsciiCodePoint::MINUS_SIGN as i32
                || x == AsciiCodePoint::UNDERSCORE as i32
                || x == AsciiCodePoint::ASTERISK as i32 =>
            {
                if let Some(existed) = marker {
                    if existed != x {
                        return None;
                    }
                } else {
                    marker = Some(x);
                }
                count += 1;
            }
            _ => return None,
        }
    }

    if count < 3 {
        return None;
    }

    let token = BlockToken::new("", THEMATIC_BREAK_TYPE, calc_line_position(line)).with_data(
        ThematicBreakTokenData {
            marker: marker?,
            continuous,
        },
    );

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
