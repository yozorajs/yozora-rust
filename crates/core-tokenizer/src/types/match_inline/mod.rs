pub mod api;
pub mod hook;

pub use api::{MatchInlineFallbackPhaseApi, MatchInlinePhaseApi};
pub use hook::{
    FindDelimiterGenerator, IsDelimiterPairResult, MatchInlineHook, MatchInlineHookCreator,
    ProcessDelimiterPairResult, ResultOfFindDelimiters, ResultOfIsDelimiterPair,
    ResultOfProcessDelimiterPair, ResultOfProcessSingleDelimiter,
};
