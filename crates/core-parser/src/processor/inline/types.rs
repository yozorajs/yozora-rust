use std::sync::Arc;

use yozora_core_tokenizer::{
    FindDelimiterGenerator, InlineToken, IsDelimiterPairResult, MatchInlineHook,
    ProcessDelimiterPairResult, TokenDelimiter,
};

pub struct MatchInlineProcessorHook<'a> {
    pub name: Arc<str>,
    pub priority: i32,
    pub hook: Box<dyn MatchInlineHook<'a> + 'a>,
    find_delimiter: Box<dyn FindDelimiterGenerator + 'a>,
}
impl<'a> MatchInlineProcessorHook<'a> {
    pub fn new(
        name: impl Into<String>,
        priority: i32,
        hook: Box<dyn MatchInlineHook<'a> + 'a>,
    ) -> Self {
        let find_delimiter = hook.find_delimiter();

        Self {
            name: Arc::<str>::from(name.into()),
            priority,
            hook,
            find_delimiter,
        }
    }

    pub fn find_delimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        self.find_delimiter.next(range_index)
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        self.hook
            .is_delimiter_pair(opener_delimiter, closer_delimiter, internal_tokens)
    }

    pub fn process_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        self.hook
            .process_delimiter_pair(opener_delimiter, closer_delimiter, internal_tokens)
    }

    pub fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        self.hook.process_single_delimiter(delimiter)
    }

    pub fn reset(&mut self) {
        self.find_delimiter = self.hook.find_delimiter();
    }
}
