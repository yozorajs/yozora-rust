use std::sync::Arc;

use yozora_ast::MATH_TYPE;
use yozora_character::{calc_trim_boundary_of_code_points, AsciiCodePoint};
use yozora_core_tokenizer::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    PhrasingContentLine, RemainingSibling,
};
use yozora_tokenizer_fenced_block::{
    fenced_block_eat_continuation_text, fenced_block_eat_opener, FencedBlockHookContext,
    FencedBlockTokenData,
};

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    let mut result = fenced_block_eat_opener(line, &create_context())?;

    // If there is no non-blank info string, it is a standard multi-line math block.
    let data = result.token.data_as::<FencedBlockTokenData>()?.clone();
    let (left, right) = calc_trim_boundary_of_code_points(&data.info_string, 0, data.info_string.len());
    if left >= right {
        return Some(result);
    }

    // Otherwise, treat it as a one-line math block wrapped by the same marker count.
    let mut i = right;
    while i > left
        && data.info_string[i - 1].code_point == AsciiCodePoint::DOLLAR_SIGN as i32
    {
        i -= 1;
    }
    let count_of_trailing_marker = right - i;
    if count_of_trailing_marker != data.marker_count {
        return None;
    }

    let lines = vec![PhrasingContentLine {
        node_points: Arc::new(data.info_string.clone()),
        start_index: 0,
        end_index: right - count_of_trailing_marker,
        first_non_whitespace_index: left,
        count_of_precede_spaces: 0,
    }];

    result.token.data = Arc::new(FencedBlockTokenData {
        marker: data.marker,
        marker_count: data.marker_count,
        indent: data.indent,
        info_string: Vec::new(),
        lines,
    });
    result.saturated = true;
    Some(result)
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
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    fenced_block_eat_continuation_text(line, token)
}

fn create_context() -> FencedBlockHookContext {
    FencedBlockHookContext {
        node_type: MATH_TYPE,
        markers: vec![AsciiCodePoint::DOLLAR_SIGN as i32],
        markers_required: 2,
        check_info_string: None,
    }
}
