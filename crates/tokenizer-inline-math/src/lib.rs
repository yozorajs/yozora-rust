mod r#match;
mod match_with_backtick;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{
    InlineMathBacktickDelimiterGenerator, InlineMathBacktickMatchHook,
    InlineMathDelimiterGenerator, InlineMathMatchHook, InlineMathParseHook,
    InlineMathPlainDelimiterGenerator, InlineMathPlainMatchHook, InlineMathTokenizer,
};
pub use types::{
    InlineMathDelimiter, InlineMathTokenData, InlineMathTokenizerOptions,
    INLINE_MATH_TOKENIZER_NAME, INLINE_MATH_TOKENIZER_NAME_WITH_BACKTICK,
    INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME,
};

pub type InlineMathToken = yozora_core_tokenizer::TypedInlineToken<InlineMathTokenData>;
pub type InlineMathHookContext = InlineMathTokenizer;
pub type InlineMathTokenizerProps = InlineMathTokenizerOptions;

pub fn inline_math_match<'a>(
    tokenizer: &'a InlineMathTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> InlineMathMatchHook<'a> {
    InlineMathMatchHook::new(tokenizer, api)
}

pub fn inline_math_match_with_backtick<'a>(
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> InlineMathBacktickMatchHook<'a> {
    InlineMathBacktickMatchHook::new(api)
}

pub fn inline_math_parse<'a>(
    tokenizer: &'a InlineMathTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> InlineMathParseHook<'a> {
    let _ = tokenizer;
    InlineMathParseHook::new(api)
}
