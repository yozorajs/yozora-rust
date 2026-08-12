mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{ThematicBreakTokenizer, THEMATIC_BREAK_TOKENIZER_NAME};

pub type ThematicBreakToken = yozora_core_tokenizer::BlockToken;
pub type ThematicBreakHookContext = ThematicBreakTokenizer;
pub type ThematicBreakTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn thematic_break_match<'a>(
    tokenizer: &'a ThematicBreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn thematic_break_parse<'a>(
    tokenizer: &'a ThematicBreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
