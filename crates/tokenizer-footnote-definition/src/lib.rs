mod r#match;
mod parse;
mod tokenizer;

pub use parse::{FootnoteDefinitionLabel, FootnoteDefinitionTokenData};
pub use r#match::eat_footnote_label;
pub use tokenizer::{FootnoteDefinitionTokenizer, FOOTNOTE_DEFINITION_TOKENIZER_NAME};

pub type FootnoteDefinitionToken =
    yozora_core_tokenizer::TypedBlockToken<FootnoteDefinitionTokenData>;
pub type FootnoteDefinitionHookContext = FootnoteDefinitionTokenizer;
pub type FootnoteDefinitionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn footnote_definition_match<'a>(
    tokenizer: &'a FootnoteDefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn footnote_definition_parse<'a>(
    tokenizer: &'a FootnoteDefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
