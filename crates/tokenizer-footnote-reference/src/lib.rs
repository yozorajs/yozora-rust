mod r#match;
mod parse;
mod tokenizer;

pub use parse::FootnoteReferenceTokenData;
pub use tokenizer::{FootnoteReferenceTokenizer, FOOTNOTE_REFERENCE_TOKENIZER_NAME};

pub type FootnoteReferenceToken =
    yozora_core_tokenizer::TypedInlineToken<FootnoteReferenceTokenData>;
pub type FootnoteReferenceHookContext = FootnoteReferenceTokenizer;
pub type FootnoteReferenceTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn footnote_reference_match<'a>(
    tokenizer: &'a FootnoteReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn footnote_reference_parse<'a>(
    tokenizer: &'a FootnoteReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
