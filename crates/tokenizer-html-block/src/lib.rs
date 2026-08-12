mod r#match;
mod parse;
mod tokenizer;
mod util;

pub use r#match::HtmlBlockTokenData;
pub use tokenizer::{HtmlBlockTokenizer, HTML_BLOCK_TOKENIZER_NAME};
pub use util::{eat_html_attribute, eat_html_tag_name, EatHtmlAttributeResult, RawHtmlAttribute};

pub type HtmlBlockToken = yozora_core_tokenizer::TypedBlockToken<HtmlBlockTokenData>;
pub type HtmlBlockHookContext = HtmlBlockTokenizer;
pub type HtmlBlockTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn html_block_match<'a>(
    tokenizer: &'a HtmlBlockTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn html_block_parse<'a>(
    tokenizer: &'a HtmlBlockTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
