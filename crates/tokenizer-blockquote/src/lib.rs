mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{BlockquoteMatchHook, BlockquoteParseHook, BlockquoteTokenizer};
pub use types::BLOCKQUOTE_TOKENIZER_NAME;

pub type BlockquoteToken = yozora_core_tokenizer::BlockToken;
pub type BlockquoteHookContext = BlockquoteTokenizer;
pub type BlockquoteTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn blockquote_match<'a>(
    tokenizer: &'a BlockquoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> BlockquoteMatchHook {
    let _ = (tokenizer, api);
    BlockquoteMatchHook::new()
}

pub fn blockquote_parse<'a>(
    tokenizer: &'a BlockquoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> BlockquoteParseHook<'a> {
    let _ = tokenizer;
    BlockquoteParseHook::new(api)
}
