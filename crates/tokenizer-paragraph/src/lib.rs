mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{ParagraphMatchHook, ParagraphParseHook, ParagraphTokenizer};
pub use types::{ParagraphTokenData, PARAGRAPH_TOKENIZER_NAME};

pub type ParagraphToken = yozora_core_tokenizer::TypedBlockToken<ParagraphTokenData>;
pub type ParagraphHookContext = ParagraphTokenizer;
pub type ParagraphTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn paragraph_match<'a>(
    tokenizer: &'a ParagraphTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> ParagraphMatchHook {
    let _ = (tokenizer, api);
    ParagraphMatchHook::new()
}

pub fn paragraph_parse<'a>(
    tokenizer: &'a ParagraphTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> ParagraphParseHook<'a> {
    let _ = tokenizer;
    ParagraphParseHook::new(api)
}
