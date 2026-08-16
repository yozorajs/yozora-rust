mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{DeleteDelimiterGenerator, DeleteMatchHook, DeleteParseHook, DeleteTokenizer};
pub use types::{DeleteDelimiter, DELETE_TOKENIZER_NAME};

pub type DeleteToken = yozora_core_tokenizer::InlineToken;
pub type DeleteHookContext = DeleteTokenizer;
pub type DeleteTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn delete_match<'a>(
    tokenizer: &'a DeleteTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> DeleteMatchHook<'a> {
    let _ = tokenizer;
    DeleteMatchHook::new(api)
}

pub fn delete_parse<'a>(
    tokenizer: &'a DeleteTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> DeleteParseHook<'a> {
    let _ = tokenizer;
    DeleteParseHook::new(api)
}
