mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{IndentedCodeMatchHook, IndentedCodeParseHook, IndentedCodeTokenizer};
pub use types::{IndentedCodeTokenData, INDENTED_CODE_TOKENIZER_NAME};

pub type IndentedCodeToken = yozora_core_tokenizer::TypedBlockToken<IndentedCodeTokenData>;
pub type IndentedCodeHookContext = IndentedCodeTokenizer;
pub type IndentedCodeTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn indented_code_match<'a>(
    tokenizer: &'a IndentedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> IndentedCodeMatchHook {
    let _ = (tokenizer, api);
    IndentedCodeMatchHook::new()
}

pub fn indented_code_parse<'a>(
    tokenizer: &'a IndentedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> IndentedCodeParseHook<'a> {
    let _ = tokenizer;
    IndentedCodeParseHook::new(api)
}
