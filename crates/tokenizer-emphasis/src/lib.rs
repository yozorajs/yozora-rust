mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{
    EmphasisDelimiterGenerator, EmphasisMatchHook, EmphasisParseHook, EmphasisTokenizer,
};
pub use types::{EmphasisDelimiter, EmphasisTokenData, EMPHASIS_TOKENIZER_NAME};

pub type EmphasisToken = yozora_core_tokenizer::TypedInlineToken<EmphasisTokenData>;
pub type EmphasisHookContext = EmphasisTokenizer;
pub type EmphasisTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn emphasis_match<'a>(
    tokenizer: &'a EmphasisTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> EmphasisMatchHook<'a> {
    let _ = tokenizer;
    EmphasisMatchHook::new(api)
}

pub fn emphasis_parse<'a>(
    tokenizer: &'a EmphasisTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> EmphasisParseHook<'a> {
    let _ = tokenizer;
    EmphasisParseHook::new(api)
}
