mod r#match;
mod parse;
mod tokenizer;

pub use parse::IndentedCodeTokenData;
pub use tokenizer::{IndentedCodeTokenizer, INDENTED_CODE_TOKENIZER_NAME};

pub type IndentedCodeToken = yozora_core_tokenizer::TypedBlockToken<IndentedCodeTokenData>;
pub type IndentedCodeHookContext = IndentedCodeTokenizer;
pub type IndentedCodeTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn indented_code_match<'a>(
    tokenizer: &'a IndentedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn indented_code_parse<'a>(
    tokenizer: &'a IndentedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
