mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{HeadingTokenizer, HEADING_TOKENIZER_NAME};

pub type HeadingToken = yozora_core_tokenizer::BlockToken;
pub type HeadingHookContext = HeadingTokenizer;
pub type HeadingTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn heading_match<'a>(
    tokenizer: &'a HeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn heading_parse<'a>(
    tokenizer: &'a HeadingTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
