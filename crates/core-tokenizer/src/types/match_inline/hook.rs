use crate::types::token::{InlineToken, TokenDelimiter};

pub trait FindDelimiterGenerator {
    fn next(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter>;
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
pub trait MatchInlineHook<'hook> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'hook>;

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
