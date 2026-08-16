mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{
    FootnoteReferenceDelimiterGenerator, FootnoteReferenceMatchHook, FootnoteReferenceParseHook,
    FootnoteReferenceTokenizer,
};
pub use types::{
    FootnoteReferenceDelimiter, FootnoteReferenceTokenData, FOOTNOTE_REFERENCE_TOKENIZER_NAME,
};

pub type FootnoteReferenceToken =
    yozora_core_tokenizer::TypedInlineToken<FootnoteReferenceTokenData>;
pub type FootnoteReferenceHookContext = FootnoteReferenceTokenizer;
pub type FootnoteReferenceTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn footnote_reference_match<'a>(
    tokenizer: &'a FootnoteReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> FootnoteReferenceMatchHook<'a> {
    let _ = tokenizer;
    FootnoteReferenceMatchHook::new(api)
}

pub fn footnote_reference_parse<'a>(
    tokenizer: &'a FootnoteReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> FootnoteReferenceParseHook<'a> {
    let _ = tokenizer;
    FootnoteReferenceParseHook::new(api)
}
