mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{TextTokenizer, TEXT_TOKENIZER_NAME};

pub type TextToken = yozora_core_tokenizer::InlineToken;
pub type TextHookContext = TextTokenizer;
pub type TextTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn text_match<'a>(
    tokenizer: &'a TextTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn text_parse<'a>(
    tokenizer: &'a TextTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
