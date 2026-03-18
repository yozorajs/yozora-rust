use yozora_character::NodePoint;

use crate::engine::token::{InlineToken, TokenDelimiter};

pub trait MatchInlinePhaseApi {
    fn has_definition(&self, identifier: &str) -> bool;

    fn has_footnote_definition(&self, identifier: &str) -> bool;

    fn get_node_points(&self) -> &[NodePoint];

    fn get_block_start_index(&self) -> usize;

    fn get_block_end_index(&self) -> usize;

    fn resolve_fallback_tokens(
        &self,
        tokens: &[InlineToken],
        token_start_index: usize,
        token_end_index: usize,
    ) -> Vec<InlineToken>;

    fn resolve_internal_tokens(
        &self,
        higher_priority_tokens: &[InlineToken],
        start_index: usize,
        end_index: usize,
    ) -> Vec<InlineToken>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsDelimiterPairResult {
    Paired,
    NotPaired { opener: bool, closer: bool },
}

#[derive(Debug, Clone)]
pub struct ProcessDelimiterPairResult {
    pub tokens: Vec<InlineToken>,
    pub remain_opener_delimiter: Option<TokenDelimiter>,
    pub remain_closer_delimiter: Option<TokenDelimiter>,
}

pub trait MatchInlineHook {
    fn reset(&mut self) {}

    fn find_delimiter(&mut self, start_index: usize, end_index: usize) -> Option<TokenDelimiter>;

    fn is_delimiter_pair(
        &self,
        _opener_delimiter: &TokenDelimiter,
        _closer_delimiter: &TokenDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        IsDelimiterPairResult::Paired
    }

    fn process_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let token_start = opener_delimiter.start_index;
        let token_end = closer_delimiter.end_index;

        let mut passthrough = Vec::with_capacity(internal_tokens.len());
        passthrough.extend(internal_tokens.iter().cloned());

        ProcessDelimiterPairResult {
            tokens: passthrough,
            remain_opener_delimiter: Some(opener_delimiter.clone()),
            remain_closer_delimiter: Some(TokenDelimiter {
                start_index: token_start,
                end_index: token_end,
                ..closer_delimiter.clone()
            }),
        }
    }

    fn process_single_delimiter(&self, _delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        Vec::new()
    }
}
