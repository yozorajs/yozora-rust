mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{FootnoteTokenizer, FOOTNOTE_TOKENIZER_NAME};

pub type FootnoteToken = yozora_core_tokenizer::InlineToken;
pub type FootnoteHookContext = FootnoteTokenizer;
pub type FootnoteTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn footnote_match<'a>(
    tokenizer: &'a FootnoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn footnote_parse<'a>(
    tokenizer: &'a FootnoteTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
