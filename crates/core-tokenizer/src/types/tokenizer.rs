use crate::constant::TokenizerType;
use crate::types::match_block::{MatchBlockHook, MatchBlockPhaseApi};
use crate::types::match_inline::{
    MatchInlineFallbackPhaseApi, MatchInlineHook, MatchInlinePhaseApi,
};
use crate::types::parse_block::{ParseBlockHook, ParseBlockPhaseApi};
use crate::types::parse_inline::{ParseInlineHook, ParseInlinePhaseApi};
use crate::types::phrasing_content::PhrasingContentLine;
use crate::types::token::{BlockToken, InlineToken};

pub trait Tokenizer {
    fn r#type(&self) -> TokenizerType;

    fn name(&self) -> &str;

    fn priority(&self) -> i32;
    fn to_string(&self) -> String {
        self.name().to_string()
    }
}

pub trait BlockTokenizer: Tokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a>;

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a>;

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

pub trait BlockFallbackTokenizer: BlockTokenizer {}

impl<T> BlockFallbackTokenizer for T where T: BlockTokenizer + ?Sized {}

pub trait InlineTokenizer: Tokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchInlinePhaseApi)
        -> Box<dyn MatchInlineHook<'a> + 'a>;

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a>;
}

pub trait InlineFallbackTokenizer: InlineTokenizer {
    fn find_and_handle_delimiter(
        &self,
        start_index: usize,
        end_index: usize,
        api: &dyn MatchInlineFallbackPhaseApi,
    ) -> InlineToken;
}
