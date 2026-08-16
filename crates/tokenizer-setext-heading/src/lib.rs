mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{SetextHeadingMatchHook, SetextHeadingParseHook, SetextHeadingTokenizer};
pub use types::{SetextHeadingTokenData, SETEXT_HEADING_TOKENIZER_NAME};

pub type SetextHeadingToken = yozora_core_tokenizer::TypedBlockToken<SetextHeadingTokenData>;
pub type SetextHeadingHookContext = SetextHeadingTokenizer;
pub type SetextHeadingTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn setext_heading_match<'a>(
    tokenizer: &'a SetextHeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> SetextHeadingMatchHook<'a> {
    let _ = tokenizer;
    SetextHeadingMatchHook::new(api)
}

pub fn setext_heading_parse<'a>(
    tokenizer: &'a SetextHeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> SetextHeadingParseHook<'a> {
    let _ = tokenizer;
    SetextHeadingParseHook::new(api)
}
