mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{AdmonitionTokenizer, ADMONITION_TOKENIZER_NAME};

pub type AdmonitionToken =
    yozora_core_tokenizer::TypedBlockToken<yozora_tokenizer_fenced_block::FencedBlockTokenData>;
pub type AdmonitionHookContext = AdmonitionTokenizer;
pub type AdmonitionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn admonition_match<'a>(
    tokenizer: &'a AdmonitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn admonition_parse<'a>(
    tokenizer: &'a AdmonitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
