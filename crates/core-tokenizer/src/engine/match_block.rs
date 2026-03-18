use crate::engine::phrasing_content::PhrasingContentLine;
use crate::engine::token::BlockToken;

pub trait MatchBlockPhaseApi {
    fn extract_phrasing_lines(&self, token: &BlockToken) -> Option<Vec<PhrasingContentLine>>;

    fn rollback_phrasing_lines(
        &self,
        lines: &[PhrasingContentLine],
        original_token: Option<&BlockToken>,
    ) -> Vec<BlockToken>;

    fn register_definition_identifier(&self, identifier: &str);

    fn register_footnote_definition_identifier(&self, identifier: &str);
}

#[derive(Debug, Clone)]
pub struct EatOpenerResult {
    pub token: BlockToken,
    pub next_index: usize,
    pub saturated: bool,
}

#[derive(Debug, Clone)]
pub enum RemainingSibling {
    None,
    One(BlockToken),
    Many(Vec<BlockToken>),
}

#[derive(Debug, Clone)]
pub struct EatAndInterruptPreviousSiblingResult {
    pub token: BlockToken,
    pub next_index: usize,
    pub saturated: bool,
    pub remaining_sibling: RemainingSibling,
}

#[derive(Debug, Clone)]
pub enum EatContinuationTextResult {
    FailedAndRollback { lines: Vec<PhrasingContentLine> },
    ClosingAndRollback { lines: Vec<PhrasingContentLine> },
    NotMatched,
    Closing { next_index: usize },
    Opening { next_index: usize },
}

#[derive(Debug, Clone)]
pub enum EatLazyContinuationTextResult {
    NotMatched,
    Opening { next_index: usize },
}

#[derive(Debug, Clone)]
pub enum OnCloseResult {
    FailedAndRollback { lines: Vec<PhrasingContentLine> },
    ClosingAndRollback { lines: Vec<PhrasingContentLine> },
}

pub trait MatchBlockHook {
    fn is_containing_block(&self) -> bool;

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        parent_token: &BlockToken,
    ) -> Option<EatOpenerResult>;

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        _line: &PhrasingContentLine,
        _prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        None
    }

    fn eat_continuation_text(
        &mut self,
        _line: &PhrasingContentLine,
        _token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        EatContinuationTextResult::NotMatched
    }

    fn eat_lazy_continuation_text(
        &mut self,
        _line: &PhrasingContentLine,
        _token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatLazyContinuationTextResult {
        EatLazyContinuationTextResult::NotMatched
    }

    fn on_close(&mut self, _token: &BlockToken) -> Option<OnCloseResult> {
        None
    }
}
