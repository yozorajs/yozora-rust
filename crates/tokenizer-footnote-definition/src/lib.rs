mod r#match;
mod parse;
mod tokenizer;
mod types;
pub mod util;

pub use tokenizer::{
    FootnoteDefinitionMatchHook, FootnoteDefinitionParseHook, FootnoteDefinitionTokenizer,
};
pub use types::{
    FootnoteDefinitionLabel, FootnoteDefinitionTokenData, FOOTNOTE_DEFINITION_TOKENIZER_NAME,
};
pub use util::eat_footnote_label;

pub type FootnoteDefinitionToken =
    yozora_core_tokenizer::TypedBlockToken<FootnoteDefinitionTokenData>;
pub type FootnoteDefinitionHookContext = FootnoteDefinitionTokenizer;
pub type FootnoteDefinitionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn footnote_definition_match<'a>(
    tokenizer: &'a FootnoteDefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> FootnoteDefinitionMatchHook<'a> {
    FootnoteDefinitionMatchHook::new(tokenizer, api)
}

pub fn footnote_definition_parse<'a>(
    tokenizer: &'a FootnoteDefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> FootnoteDefinitionParseHook<'a> {
    let _ = tokenizer;
    FootnoteDefinitionParseHook::new(api)
}
