mod r#match;
mod parse;
mod tokenizer;

pub use parse::InlineMathTokenData;
pub use tokenizer::{
    InlineMathTokenizer, InlineMathTokenizerOptions, INLINE_MATH_TOKENIZER_NAME,
    INLINE_MATH_TOKENIZER_NAME_WITH_BACKTICK, INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME,
};

pub type InlineMathToken = yozora_core_tokenizer::TypedInlineToken<InlineMathTokenData>;
pub type InlineMathHookContext = InlineMathTokenizer;
pub type InlineMathTokenizerProps = InlineMathTokenizerOptions;

pub fn inline_math_match<'a>(
    tokenizer: &'a InlineMathTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn inline_math_match_with_backtick<'a>(
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    tokenizer::match_with_backtick(api)
}

pub fn inline_math_parse<'a>(
    tokenizer: &'a InlineMathTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
