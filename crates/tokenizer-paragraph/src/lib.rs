mod r#match;
mod parse;
mod tokenizer;

pub use r#match::ParagraphTokenData;
pub use tokenizer::{ParagraphTokenizer, PARAGRAPH_TOKENIZER_NAME};

pub type ParagraphToken = yozora_core_tokenizer::TypedBlockToken<ParagraphTokenData>;
pub type ParagraphHookContext = ParagraphTokenizer;
pub type ParagraphTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn paragraph_match<'a>(
    tokenizer: &'a ParagraphTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn paragraph_parse<'a>(
    tokenizer: &'a ParagraphTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
