mod r#match;
mod parse;
mod tokenizer;
mod util;

pub use tokenizer::{ImageTokenizer, IMAGE_TOKENIZER_NAME};
pub use util::calc_image_alt;

pub type ImageToken = yozora_core_tokenizer::InlineToken;
pub type ImageHookContext = ImageTokenizer;
pub type ImageTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn image_match<'a>(
    tokenizer: &'a ImageTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn image_parse<'a>(
    tokenizer: &'a ImageTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
