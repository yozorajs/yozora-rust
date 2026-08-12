mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{ImageReferenceTokenizer, IMAGE_REFERENCE_TOKENIZER_NAME};

pub type ImageReferenceToken = yozora_core_tokenizer::InlineToken;
pub type ImageReferenceHookContext = ImageReferenceTokenizer;
pub type ImageReferenceTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn image_reference_match<'a>(
    tokenizer: &'a ImageReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn image_reference_parse<'a>(
    tokenizer: &'a ImageReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
