mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{
    LinkReferenceDelimiterGenerator, LinkReferenceMatchHook, LinkReferenceParseHook,
    LinkReferenceTokenizer,
};
pub use types::{
    LinkReferenceDelimiter, LinkReferenceDelimiterBracket, LinkReferenceProcessDelimiterPairResult,
    LinkReferenceTokenData, LINK_REFERENCE_TOKENIZER_NAME,
};

pub type LinkReferenceToken = yozora_core_tokenizer::TypedInlineToken<LinkReferenceTokenData>;
pub type LinkReferenceHookContext = LinkReferenceTokenizer;
pub type LinkReferenceTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn link_reference_match<'a>(
    tokenizer: &'a LinkReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> LinkReferenceMatchHook<'a> {
    let _ = tokenizer;
    LinkReferenceMatchHook::new(api)
}

pub fn link_reference_parse<'a>(
    tokenizer: &'a LinkReferenceTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> LinkReferenceParseHook<'a> {
    let _ = tokenizer;
    LinkReferenceParseHook::new(api)
}
