mod r#match;
mod parse;
mod tokenizer;

pub use r#match::SetextHeadingTokenData;
pub use tokenizer::{SetextHeadingTokenizer, SETEXT_HEADING_TOKENIZER_NAME};

pub type SetextHeadingToken = yozora_core_tokenizer::TypedBlockToken<SetextHeadingTokenData>;
pub type SetextHeadingHookContext = SetextHeadingTokenizer;
pub type SetextHeadingTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn setext_heading_match<'a>(
    tokenizer: &'a SetextHeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn setext_heading_parse<'a>(
    tokenizer: &'a SetextHeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
