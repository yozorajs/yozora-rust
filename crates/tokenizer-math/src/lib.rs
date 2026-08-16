mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{MathMatchHook, MathParseHook, MathTokenizer};
pub use types::MATH_TOKENIZER_NAME;

pub type MathToken =
    yozora_core_tokenizer::TypedBlockToken<yozora_tokenizer_fenced_block::FencedBlockTokenData>;
pub type MathHookContext = MathTokenizer;
pub type MathTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn math_match<'a>(
    tokenizer: &'a MathTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> MathMatchHook {
    let _ = (tokenizer, api);
    MathMatchHook::new()
}

pub fn math_parse<'a>(
    tokenizer: &'a MathTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> MathParseHook<'a> {
    let _ = tokenizer;
    MathParseHook::new(api)
}
