mod r#match;
mod tokenizer;

pub use r#match::{
    eat_and_interrupt_previous_sibling as fenced_block_eat_and_interrupt_previous_sibling,
    eat_continuation_text as fenced_block_eat_continuation_text,
    eat_opener as fenced_block_eat_opener, fenced_block_match, CheckInfoStringFn,
    FencedBlockHookContext, FencedBlockMatchHook, FencedBlockTokenData,
};
pub use tokenizer::{
    FencedBlockTokenizer, FencedBlockTokenizerOptions, FENCED_BLOCK_TOKENIZER_NAME,
    FENCED_BLOCK_TYPE,
};

pub type FencedBlockToken = yozora_core_tokenizer::TypedBlockToken<FencedBlockTokenData>;
pub type FencedBlockTokenizerProps = FencedBlockTokenizerOptions;
