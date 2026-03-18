use crate::engine::constant::TokenizerType;
use crate::engine::match_block::{MatchBlockHook, MatchBlockPhaseApi};
use crate::engine::match_inline::{MatchInlineHook, MatchInlinePhaseApi};
use crate::engine::parse_block::{ParseBlockHook, ParseBlockPhaseApi};
use crate::engine::parse_inline::{ParseInlineHook, ParseInlinePhaseApi};
use crate::engine::phrasing_content::PhrasingContentLine;
use crate::engine::token::{BlockToken, InlineToken};

pub trait EngineTokenizer {
    fn tokenizer_type(&self) -> TokenizerType;

    fn name(&self) -> &str;

    fn priority(&self) -> i32;
}

pub trait EngineBlockTokenizer: EngineTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn MatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a>;

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn ParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a>;

    fn extract_phrasing_content_lines(
        &self,
        _token: &BlockToken,
    ) -> Option<Vec<PhrasingContentLine>> {
        None
    }

    fn build_block_token(
        &self,
        _lines: &[PhrasingContentLine],
        _original_token: &BlockToken,
    ) -> Option<BlockToken> {
        None
    }
}

pub trait EngineInlineTokenizer: EngineTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a>;

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a>;
}

pub trait EngineInlineFallbackTokenizer: EngineInlineTokenizer {
    fn find_and_handle_delimiter(
        &self,
        start_index: usize,
        end_index: usize,
        api: &dyn MatchInlinePhaseApi,
    ) -> InlineToken;
}
