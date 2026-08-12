mod r#match;
mod parse;
mod tokenizer;

pub use r#match::LinkReferenceDelimiterBracket;
pub use tokenizer::{LinkReferenceTokenizer, LINK_REFERENCE_TOKENIZER_NAME};

pub type LinkReferenceToken = yozora_core_tokenizer::InlineToken;
pub type LinkReferenceHookContext = LinkReferenceTokenizer;
pub type LinkReferenceTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn link_reference_match<'a>(
    tokenizer: &'a LinkReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn link_reference_parse<'a>(
    tokenizer: &'a LinkReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
