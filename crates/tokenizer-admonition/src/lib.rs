mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{AdmonitionMatchHook, AdmonitionParseHook, AdmonitionTokenizer};
pub use types::ADMONITION_TOKENIZER_NAME;

pub type AdmonitionToken =
    yozora_core_tokenizer::TypedBlockToken<yozora_tokenizer_fenced_block::FencedBlockTokenData>;
pub type AdmonitionHookContext = AdmonitionTokenizer;
pub type AdmonitionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn admonition_match<'a>(
    tokenizer: &'a AdmonitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> AdmonitionMatchHook<'a> {
    let _ = tokenizer;
    AdmonitionMatchHook::new(api)
}

pub fn admonition_parse<'a>(
    tokenizer: &'a AdmonitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> AdmonitionParseHook<'a> {
    let _ = tokenizer;
    AdmonitionParseHook::new(api)
}
