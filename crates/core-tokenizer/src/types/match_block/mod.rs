pub mod api;
pub mod hook;

pub use api::MatchBlockPhaseApi;
pub use hook::{
    EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatLazyContinuationTextResult,
    EatOpenerResult, MatchBlockHook, OnCloseResult, RemainingSibling,
};
