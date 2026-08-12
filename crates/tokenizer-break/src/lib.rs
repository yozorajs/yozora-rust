mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{BreakTokenizer, BREAK_TOKENIZER_NAME};

pub type BreakToken = yozora_core_tokenizer::InlineToken;
pub type BreakHookContext = BreakTokenizer;
pub type BreakTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn break_match<'a>(
    tokenizer: &'a BreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn break_parse<'a>(
    tokenizer: &'a BreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
