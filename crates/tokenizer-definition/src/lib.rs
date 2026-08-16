mod r#match;
mod parse;
mod tokenizer;
mod types;
pub mod util;

pub use tokenizer::{DefinitionMatchHook, DefinitionParseHook, DefinitionTokenizer};
pub use types::{
    CollectResult, DefinitionTokenData, LinkDestinationCollectingState, LinkLabelCollectingState,
    LinkTitleCollectingState, DEFINITION_TOKENIZER_NAME,
};
pub use util::{
    eat_and_collect_link_destination, eat_and_collect_link_label, eat_and_collect_link_title,
};

pub type DefinitionToken = yozora_core_tokenizer::TypedBlockToken<DefinitionTokenData>;
pub type DefinitionHookContext = DefinitionTokenizer;
pub type DefinitionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn definition_match<'a>(
    tokenizer: &'a DefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> DefinitionMatchHook<'a> {
    let _ = tokenizer;
    DefinitionMatchHook::new(api)
}

pub fn definition_parse<'a>(
    tokenizer: &'a DefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> DefinitionParseHook<'a> {
    let _ = tokenizer;
    DefinitionParseHook::new(api)
}
