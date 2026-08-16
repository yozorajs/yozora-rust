mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{
    InlineCodeDelimiterGenerator, InlineCodeMatchHook, InlineCodeParseHook, InlineCodeTokenizer,
};
pub use types::{InlineCodeDelimiter, InlineCodeTokenData, INLINE_CODE_TOKENIZER_NAME};

pub type InlineCodeToken = yozora_core_tokenizer::TypedInlineToken<InlineCodeTokenData>;
pub type InlineCodeHookContext = InlineCodeTokenizer;
pub type InlineCodeTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn inline_code_match<'a>(
    tokenizer: &'a InlineCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> InlineCodeMatchHook<'a> {
    let _ = tokenizer;
    InlineCodeMatchHook::new(api)
}

pub fn inline_code_parse<'a>(
    tokenizer: &'a InlineCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> InlineCodeParseHook<'a> {
    let _ = tokenizer;
    InlineCodeParseHook::new(api)
}
