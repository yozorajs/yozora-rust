mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{FencedCodeTokenizer, FENCED_CODE_TOKENIZER_NAME};

pub type FencedCodeToken =
    yozora_core_tokenizer::TypedBlockToken<yozora_tokenizer_fenced_block::FencedBlockTokenData>;
pub type FencedCodeHookContext = FencedCodeTokenizer;
pub type FencedCodeTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn fenced_code_match<'a>(
    tokenizer: &'a FencedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn fenced_code_parse<'a>(
    tokenizer: &'a FencedCodeTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
