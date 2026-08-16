use yozora_ast::BLOCKQUOTE_TYPE;
use yozora_character::{is_space_character, AsciiCodePoint};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, eat_indentation, BlockToken,
    EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    PhrasingContentLine, RemainingSibling,
};

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width >= 4 {
        return None;
    }

    let first = line.first_non_whitespace_index;
    if first >= line.end_index {
        return None;
    }

    if line.node_points[first].code_point != AsciiCodePoint::CLOSE_ANGLE as i32 {
        return None;
    }

    let next_index = calc_blockquote_marker_end(line.node_points.as_ref(), first, line.end_index);

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

    if line.indent_width >= 4
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

    let next_index = calc_blockquote_marker_end(line.node_points.as_ref(), first, line.end_index);

    EatContinuationTextResult::Opening { next_index }
}

fn calc_blockquote_marker_end(
    node_points: &[yozora_character::NodePoint],
    marker_index: usize,
    end_index: usize,
) -> usize {
    let mut next_index = marker_index + 1;
    if next_index < end_index && is_space_character(node_points[next_index].code_point) {
        next_index = eat_indentation(node_points, next_index, end_index, 1).unwrap_or(next_index);
    }
    next_index
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
