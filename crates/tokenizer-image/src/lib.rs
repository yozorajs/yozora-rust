mod r#match;
mod parse;
mod tokenizer;
mod types;
mod util;

pub use tokenizer::{ImageDelimiterGenerator, ImageMatchHook, ImageParseHook, ImageTokenizer};
pub use types::{ImageDelimiter, ImageTokenData, IMAGE_TOKENIZER_NAME};
pub use util::calc_image_alt;

pub type ImageToken = yozora_core_tokenizer::TypedInlineToken<ImageTokenData>;
pub type ImageHookContext = ImageTokenizer;
pub type ImageTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn image_match<'a>(
    tokenizer: &'a ImageTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> ImageMatchHook<'a> {
    let _ = tokenizer;
    ImageMatchHook::new(api)
}

pub fn image_parse<'a>(
    tokenizer: &'a ImageTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> ImageParseHook<'a> {
    let _ = tokenizer;
    ImageParseHook::new(api)
}
