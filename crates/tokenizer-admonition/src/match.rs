use yozora_ast::ADMONITION_TYPE;
use yozora_character::AsciiCodePoint;
use yozora_core_tokenizer::{EatContinuationTextResult, EatOpenerResult, PhrasingContentLine};
use yozora_tokenizer_fenced_block::{
    fenced_block_eat_continuation_text, fenced_block_eat_opener, FencedBlockHookContext,
};

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    fenced_block_eat_opener(line, &create_context())
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut yozora_core_tokenizer::BlockToken,
) -> EatContinuationTextResult {
    fenced_block_eat_continuation_text(line, token)
}

fn create_context() -> FencedBlockHookContext {
    FencedBlockHookContext {
        node_type: ADMONITION_TYPE,
        markers: vec![AsciiCodePoint::COLON as i32],
        markers_required: 3,
        check_info_string: None,
    }
}
