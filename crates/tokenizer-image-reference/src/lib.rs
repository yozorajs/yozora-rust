mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{
    ImageReferenceDelimiterGenerator, ImageReferenceMatchHook, ImageReferenceParseHook,
    ImageReferenceTokenizer,
};
pub use types::{ImageReferenceDelimiter, ImageReferenceTokenData, IMAGE_REFERENCE_TOKENIZER_NAME};

pub type ImageReferenceToken = yozora_core_tokenizer::TypedInlineToken<ImageReferenceTokenData>;
pub type ImageReferenceHookContext = ImageReferenceTokenizer;
pub type ImageReferenceTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn image_reference_match<'a>(
    tokenizer: &'a ImageReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> ImageReferenceMatchHook<'a> {
    let _ = tokenizer;
    ImageReferenceMatchHook::new(api)
}

pub fn image_reference_parse<'a>(
    tokenizer: &'a ImageReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> ImageReferenceParseHook<'a> {
    let _ = tokenizer;
    ImageReferenceParseHook::new(api)
}
