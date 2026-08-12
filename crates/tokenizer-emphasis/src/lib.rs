mod r#match;
mod parse;
mod tokenizer;

pub use parse::EmphasisTokenData;
pub use tokenizer::{EmphasisTokenizer, EMPHASIS_TOKENIZER_NAME};

pub type EmphasisToken = yozora_core_tokenizer::TypedInlineToken<EmphasisTokenData>;
pub type EmphasisHookContext = EmphasisTokenizer;
pub type EmphasisTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn emphasis_match<'a>(
    tokenizer: &'a EmphasisTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn emphasis_parse<'a>(
    tokenizer: &'a EmphasisTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
