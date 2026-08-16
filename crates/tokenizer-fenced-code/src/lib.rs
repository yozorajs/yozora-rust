mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{FencedCodeMatchHook, FencedCodeParseHook, FencedCodeTokenizer};
pub use types::FENCED_CODE_TOKENIZER_NAME;

pub type FencedCodeToken =
    yozora_core_tokenizer::TypedBlockToken<yozora_tokenizer_fenced_block::FencedBlockTokenData>;
pub type FencedCodeHookContext = FencedCodeTokenizer;
pub type FencedCodeTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn fenced_code_match<'a>(
    tokenizer: &'a FencedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> FencedCodeMatchHook {
    let _ = (tokenizer, api);
    FencedCodeMatchHook::new()
}

pub fn fenced_code_parse<'a>(
    tokenizer: &'a FencedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> FencedCodeParseHook<'a> {
    let _ = tokenizer;
    FencedCodeParseHook::new(api)
}
