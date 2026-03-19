use std::sync::Arc;

use yozora_ast::CODE_TYPE;
use yozora_character::AsciiCodePoint;
use yozora_core_tokenizer::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    PhrasingContentLine,
};
use yozora_tokenizer_fenced_block::{
    fenced_block_eat_and_interrupt_previous_sibling, fenced_block_eat_continuation_text,
    fenced_block_eat_opener, FencedBlockHookContext,
};

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    fenced_block_eat_opener(line, &create_context())
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    fenced_block_eat_and_interrupt_previous_sibling(line, prev_sibling_token, &create_context())
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    fenced_block_eat_continuation_text(line, token)
}

fn create_context() -> FencedBlockHookContext {
    FencedBlockHookContext {
        node_type: CODE_TYPE,
        markers: vec![AsciiCodePoint::BACKTICK as i32, AsciiCodePoint::TILDE as i32],
        markers_required: 3,
        check_info_string: Some(Arc::new(|info_string, marker, _marker_count| {
            // Backtick fenced code info string cannot contain backticks.
            if marker != AsciiCodePoint::BACKTICK as i32 {
                return true;
            }
            !info_string
                .iter()
                .any(|point| point.code_point == AsciiCodePoint::BACKTICK as i32)
        })),
    }
}
