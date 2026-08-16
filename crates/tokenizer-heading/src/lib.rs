mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{HeadingMatchHook, HeadingParseHook, HeadingTokenizer};
pub use types::{HeadingTokenData, HEADING_TOKENIZER_NAME};

pub type HeadingToken = yozora_core_tokenizer::TypedBlockToken<HeadingTokenData>;
pub type HeadingHookContext = HeadingTokenizer;
pub type HeadingTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn heading_match<'a>(
    tokenizer: &'a HeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> HeadingMatchHook {
    let _ = (tokenizer, api);
    HeadingMatchHook::new()
}

pub fn heading_parse<'a>(
    tokenizer: &'a HeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> HeadingParseHook<'a> {
    let _ = tokenizer;
    HeadingParseHook::new(api)
}
