mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{TextDelimiterGenerator, TextMatchHook, TextParseHook, TextTokenizer};
pub use types::{TextDelimiter, TEXT_TOKENIZER_NAME};

pub type TextToken = yozora_core_tokenizer::InlineToken;
pub type TextHookContext = TextTokenizer;
pub type TextTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn text_match<'a>(
    tokenizer: &'a TextTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> TextMatchHook {
    let _ = (tokenizer, api);
    TextMatchHook::new()
}

pub fn text_parse<'a>(
    tokenizer: &'a TextTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> TextParseHook<'a> {
    let _ = tokenizer;
    TextParseHook::new(api)
}
