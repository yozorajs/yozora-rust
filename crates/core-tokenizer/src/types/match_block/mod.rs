pub mod api;
pub mod hook;

pub use api::MatchBlockPhaseApi;
pub use hook::{
    EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatLazyContinuationTextResult,
    EatOpenerResult, MatchBlockHook, MatchBlockHookCreator, OnCloseResult, RemainingSibling,
    ResultOfEatAndInterruptPreviousSibling, ResultOfEatContinuationText,
    ResultOfEatLazyContinuationText, ResultOfEatOpener, ResultOfOnClose,
};
