use std::sync::Arc;

use yozora_core_tokenizer::{
    FindDelimiterGenerator, InlineToken, IsDelimiterPairResult, MatchInlineHook,
    ProcessDelimiterPairResult, TokenDelimiter, TokenizerId,
};

pub struct MatchInlineProcessorHook<'a> {
    pub name: Arc<str>,
    pub tokenizer_id: TokenizerId,
    pub priority: i32,
    pub hook: Box<dyn MatchInlineHook<'a> + 'a>,
    find_delimiter: Box<dyn FindDelimiterGenerator + 'a>,
}

#[allow(non_snake_case)]
impl<'a> MatchInlineProcessorHook<'a> {
    pub fn new(
        name: impl Into<String>,
        tokenizer_id: TokenizerId,
        priority: i32,
        hook: Box<dyn MatchInlineHook<'a> + 'a>,
    ) -> Self {
        let find_delimiter = hook.findDelimiter();

        Self {
            name: Arc::<str>::from(name.into()),
            tokenizer_id,
            priority,
            hook,
            find_delimiter,
        }
    }

    pub fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        self.find_delimiter.next(range_index)
    }

    pub fn isDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        self.hook
            .isDelimiterPair(opener_delimiter, closer_delimiter, internal_tokens)
    }

    pub fn processDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        self.hook
            .processDelimiterPair(opener_delimiter, closer_delimiter, internal_tokens)
    }

    pub fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        self.hook.processSingleDelimiter(delimiter)
    }

    pub fn reset(&mut self) {
        self.find_delimiter = self.hook.findDelimiter();
    }
}
