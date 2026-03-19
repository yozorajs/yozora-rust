use yozora_core_tokenizer::{
    genFindDelimiter, InlineToken, IsDelimiterPairResult, MatchInlineHook,
    ProcessDelimiterPairResult, TokenDelimiter,
};

pub struct MatchInlineProcessorHook<'a> {
    pub name: String,
    pub priority: i32,
    pub hook: Box<dyn MatchInlineHook + 'a>,
    find_delimiter_last_end_index: Option<usize>,
    find_delimiter_last_delimiter: Option<TokenDelimiter>,
}

#[allow(non_snake_case)]
impl<'a> MatchInlineProcessorHook<'a> {
    pub fn new(
        name: impl Into<String>,
        priority: i32,
        hook: Box<dyn MatchInlineHook + 'a>,
    ) -> Self {
        Self {
            name: name.into(),
            priority,
            hook,
            find_delimiter_last_end_index: None,
            find_delimiter_last_delimiter: None,
        }
    }

    pub fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        genFindDelimiter(
            range_index,
            &mut self.find_delimiter_last_end_index,
            &mut self.find_delimiter_last_delimiter,
            |start_index, end_index| self.hook.findDelimiter((start_index, end_index)),
        )
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
        self.find_delimiter_last_end_index = None;
        self.find_delimiter_last_delimiter = None;
        self.hook.reset();
    }
}
