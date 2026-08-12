mod r#match;
mod parse;
mod tokenizer;
mod util;

pub use r#match::HtmlInlineTokenData;
pub use tokenizer::{HtmlInlineTokenizer, HTML_INLINE_TOKENIZER_NAME};
pub use util::*;

pub type HtmlInlineToken = yozora_core_tokenizer::TypedInlineToken<HtmlInlineTokenData>;
pub type HtmlInlineHookContext = HtmlInlineTokenizer;
pub type HtmlInlineTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn html_inline_match<'a>(
    tokenizer: &'a HtmlInlineTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn html_inline_parse<'a>(
    tokenizer: &'a HtmlInlineTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
