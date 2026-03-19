use yozora_ast::BLOCKQUOTE_TYPE;
use yozora_character::{is_space_character, AsciiCodePoint, VirtualCodePoint};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, EatAndInterruptPreviousSiblingResult,
    EatContinuationTextResult, EatOpenerResult, PhrasingContentLine, RemainingSibling,
};

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.count_of_precede_spaces >= 4 {
        return None;
    }

    let first = line.first_non_whitespace_index;
    if first >= line.end_index {
        return None;
    }

    if line.node_points[first].code_point != AsciiCodePoint::CLOSE_ANGLE as i32 {
        return None;
    }

    let mut next_index = first + 1;
    if next_index < line.end_index && is_space_character(line.node_points[next_index].code_point) {
        next_index += 1;
        if next_index < line.end_index
            && line.node_points[next_index].code_point == VirtualCodePoint::Space as i32
        {
            next_index += 1;
        }
    }

    let token = BlockToken::new(
        "",
        BLOCKQUOTE_TYPE,
        calc_segment_position(line, line.start_index, next_index),
    );
    Some(EatOpenerResult {
        token,
        next_index,
        saturated: false,
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

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    parent_token: &BlockToken,
) -> EatContinuationTextResult {
    let first = line.first_non_whitespace_index;
    let marker = line
        .node_points
        .get(first)
        .map(|point| point.code_point)
        .unwrap_or_default();

    if line.count_of_precede_spaces >= 4
        || first >= line.end_index
        || marker != AsciiCodePoint::CLOSE_ANGLE as i32
    {
        if parent_token.node_type == BLOCKQUOTE_TYPE {
            return EatContinuationTextResult::Opening {
                next_index: line.start_index,
            };
        }
        return EatContinuationTextResult::NotMatched;
    }

    let mut next_index = first + 1;
    if next_index < line.end_index && is_space_character(line.node_points[next_index].code_point) {
        next_index += 1;
    }

    EatContinuationTextResult::Opening { next_index }
}

fn calc_segment_position(
    line: &PhrasingContentLine,
    start_index: usize,
    end_index: usize,
) -> Option<yozora_ast::Position> {
    if start_index >= end_index {
        return None;
    }

    Some(yozora_ast::Position {
        start: calc_start_point(line.node_points.as_ref(), start_index),
        end: calc_end_point(line.node_points.as_ref(), end_index - 1),
        indent: None,
    })
}
