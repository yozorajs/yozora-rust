mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{ThematicBreakMatchHook, ThematicBreakParseHook, ThematicBreakTokenizer};
pub use types::{ThematicBreakTokenData, THEMATIC_BREAK_TOKENIZER_NAME};

pub type ThematicBreakToken = yozora_core_tokenizer::TypedBlockToken<ThematicBreakTokenData>;
pub type ThematicBreakHookContext = ThematicBreakTokenizer;
pub type ThematicBreakTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn thematic_break_match<'a>(
    tokenizer: &'a ThematicBreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> ThematicBreakMatchHook {
    let _ = (tokenizer, api);
    ThematicBreakMatchHook::new()
}

pub fn thematic_break_parse<'a>(
    tokenizer: &'a ThematicBreakTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> ThematicBreakParseHook<'a> {
    let _ = tokenizer;
    ThematicBreakParseHook::new(api)
}
