use crate::types::token::{InlineToken, TokenDelimiter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsDelimiterPairResult {
    Paired,
    NotPaired { opener: bool, closer: bool },
}

#[derive(Debug, Clone)]
#[allow(non_snake_case)]
pub struct ProcessDelimiterPairResult {
    pub tokens: Vec<InlineToken>,
    pub remainOpenerDelimiter: Option<TokenDelimiter>,
    pub remainCloserDelimiter: Option<TokenDelimiter>,
}

#[allow(non_snake_case)]
pub trait MatchInlineHook {
    fn reset(&mut self) {}

    fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter>;

    fn isDelimiterPair(
        &self,
        _opener_delimiter: &TokenDelimiter,
        _closer_delimiter: &TokenDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        IsDelimiterPairResult::Paired
    }

    fn processDelimiterPair(
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
            remainOpenerDelimiter: Some(opener_delimiter.clone()),
            remainCloserDelimiter: Some(TokenDelimiter {
                start_index: token_start,
                end_index: token_end,
                ..closer_delimiter.clone()
            }),
        }
    }

    fn processSingleDelimiter(&self, _delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        Vec::new()
    }
}
