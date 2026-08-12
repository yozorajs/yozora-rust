mod r#match;
mod parse;
mod tokenizer;

pub use parse::InlineCodeTokenData;
pub use tokenizer::{InlineCodeTokenizer, INLINE_CODE_TOKENIZER_NAME};

pub type InlineCodeToken = yozora_core_tokenizer::TypedInlineToken<InlineCodeTokenData>;
pub type InlineCodeHookContext = InlineCodeTokenizer;
pub type InlineCodeTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn inline_code_match<'a>(
    tokenizer: &'a InlineCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn inline_code_parse<'a>(
    tokenizer: &'a InlineCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
