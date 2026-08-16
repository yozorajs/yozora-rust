mod r#match;
mod parse;
mod tokenizer;
mod types;
mod util;

pub use tokenizer::{
    HtmlInlineDelimiterGenerator, HtmlInlineMatchHook, HtmlInlineParseHook, HtmlInlineTokenizer,
};
pub use types::{HtmlInlineDelimiter, HtmlInlineTokenData, HTML_INLINE_TOKENIZER_NAME};
pub use util::*;

pub type HtmlInlineToken = yozora_core_tokenizer::TypedInlineToken<HtmlInlineTokenData>;
pub type HtmlInlineHookContext = HtmlInlineTokenizer;
pub type HtmlInlineTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn html_inline_match<'a>(
    tokenizer: &'a HtmlInlineTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> HtmlInlineMatchHook<'a> {
    let _ = tokenizer;
    HtmlInlineMatchHook::new(api)
}

pub fn html_inline_parse<'a>(
    tokenizer: &'a HtmlInlineTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> HtmlInlineParseHook<'a> {
    let _ = tokenizer;
    HtmlInlineParseHook::new(api)
}
