mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{BlockquoteTokenizer, BLOCKQUOTE_TOKENIZER_NAME};

pub type BlockquoteToken = yozora_core_tokenizer::BlockToken;
pub type BlockquoteHookContext = BlockquoteTokenizer;
pub type BlockquoteTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn blockquote_match<'a>(
    tokenizer: &'a BlockquoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn blockquote_parse<'a>(
    tokenizer: &'a BlockquoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
