mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{MathTokenizer, MATH_TOKENIZER_NAME};

pub type MathToken = yozora_core_tokenizer::BlockToken;
pub type MathHookContext = MathTokenizer;
pub type MathTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn math_match<'a>(
    tokenizer: &'a MathTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn math_parse<'a>(
    tokenizer: &'a MathTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
