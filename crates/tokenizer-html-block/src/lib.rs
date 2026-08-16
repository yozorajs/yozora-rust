mod conditions;
mod r#match;
mod parse;
mod tokenizer;
mod types;
mod util;

pub use tokenizer::{HtmlBlockMatchHook, HtmlBlockParseHook, HtmlBlockTokenizer};
pub use types::{HtmlBlockConditionType, HtmlBlockTokenData, HTML_BLOCK_TOKENIZER_NAME};
pub use util::{eat_html_attribute, eat_html_tag_name, EatHtmlAttributeResult, RawHtmlAttribute};

pub type HtmlBlockToken = yozora_core_tokenizer::TypedBlockToken<HtmlBlockTokenData>;
pub type HtmlBlockHookContext = HtmlBlockTokenizer;
pub type HtmlBlockTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn html_block_match<'a>(
    tokenizer: &'a HtmlBlockTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> HtmlBlockMatchHook {
    let _ = (tokenizer, api);
    HtmlBlockMatchHook::new()
}

pub fn html_block_parse<'a>(
    tokenizer: &'a HtmlBlockTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> HtmlBlockParseHook<'a> {
    let _ = tokenizer;
    HtmlBlockParseHook::new(api)
}
