mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{
    FootnoteDelimiterGenerator, FootnoteMatchHook, FootnoteParseHook, FootnoteTokenizer,
};
pub use types::{FootnoteDelimiter, FOOTNOTE_TOKENIZER_NAME};

pub type FootnoteToken = yozora_core_tokenizer::InlineToken;
pub type FootnoteHookContext = FootnoteTokenizer;
pub type FootnoteTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn footnote_match<'a>(
    tokenizer: &'a FootnoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> FootnoteMatchHook<'a> {
    let _ = tokenizer;
    FootnoteMatchHook::new(api)
}

pub fn footnote_parse<'a>(
    tokenizer: &'a FootnoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> FootnoteParseHook<'a> {
    let _ = tokenizer;
    FootnoteParseHook::new(api)
}
