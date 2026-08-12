mod r#match;
mod parse;
mod tokenizer;

pub use r#match::{
    eat_and_collect_link_destination, eat_and_collect_link_label, eat_and_collect_link_title,
    CollectResult, DefinitionTokenData, LinkDestinationCollectingState, LinkLabelCollectingState,
    LinkTitleCollectingState,
};
pub use tokenizer::{DefinitionTokenizer, DEFINITION_TOKENIZER_NAME};

pub type DefinitionToken = yozora_core_tokenizer::TypedBlockToken<DefinitionTokenData>;
pub type DefinitionHookContext = DefinitionTokenizer;
pub type DefinitionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn definition_match<'a>(
    tokenizer: &'a DefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn definition_parse<'a>(
    tokenizer: &'a DefinitionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
