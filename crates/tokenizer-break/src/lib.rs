mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{BreakDelimiterGenerator, BreakMatchHook, BreakParseHook, BreakTokenizer};
pub use types::{BreakDelimiter, BreakTokenMarkerType, BREAK_TOKENIZER_NAME};

pub type BreakToken = yozora_core_tokenizer::InlineToken;
pub type BreakHookContext = BreakTokenizer;
pub type BreakTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn break_match<'a>(
    tokenizer: &'a BreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> BreakMatchHook<'a> {
    let _ = tokenizer;
    BreakMatchHook::new(api)
}

pub fn break_parse<'a>(
    tokenizer: &'a BreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> BreakParseHook<'a> {
    let _ = tokenizer;
    BreakParseHook::new(api)
}
