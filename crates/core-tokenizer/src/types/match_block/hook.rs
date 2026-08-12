use crate::types::match_block::MatchBlockPhaseApi;
use crate::types::phrasing_content::PhrasingContentLine;
use crate::types::token::BlockToken;

pub type MatchBlockHookCreator<'a> =
    dyn Fn(&'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> + 'a;

#[derive(Debug, Clone)]
pub struct EatOpenerResult {
    pub token: BlockToken,
    pub next_index: usize,
    pub saturated: bool,
}

pub type ResultOfEatOpener = Option<EatOpenerResult>;

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

pub type ResultOfEatAndInterruptPreviousSibling = Option<EatAndInterruptPreviousSiblingResult>;

#[derive(Debug, Clone)]
pub enum EatContinuationTextResult {
    FailedAndRollback { lines: Vec<PhrasingContentLine> },
    ClosingAndRollback { lines: Vec<PhrasingContentLine> },
    NotMatched,
    Closing { next_index: usize },
    Opening { next_index: usize },
}

pub type ResultOfEatContinuationText = EatContinuationTextResult;

#[derive(Debug, Clone)]
pub enum EatLazyContinuationTextResult {
    NotMatched,
    Opening { next_index: usize },
}

pub type ResultOfEatLazyContinuationText = EatLazyContinuationTextResult;

#[derive(Debug, Clone)]
pub enum OnCloseResult {
    FailedAndRollback { lines: Vec<PhrasingContentLine> },
    ClosingAndRollback { lines: Vec<PhrasingContentLine> },
}

pub type ResultOfOnClose = Option<OnCloseResult>;

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

    fn on_close(&mut self, _token: &mut BlockToken) -> Option<OnCloseResult> {
        None
    }
}
