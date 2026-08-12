mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{DeleteTokenizer, DELETE_TOKENIZER_NAME};

pub type DeleteToken = yozora_core_tokenizer::InlineToken;
pub type DeleteHookContext = DeleteTokenizer;
pub type DeleteTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn delete_match<'a>(
    tokenizer: &'a DeleteTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn delete_parse<'a>(
    tokenizer: &'a DeleteTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
