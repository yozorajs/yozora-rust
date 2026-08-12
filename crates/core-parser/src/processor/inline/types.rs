use std::sync::Arc;

use yozora_core_tokenizer::{
    FindDelimiterGenerator, InlineToken, IsDelimiterPairResult, MatchInlineFallbackPhaseApi,
    MatchInlineHook, ProcessDelimiterPairResult, TokenDelimiter,
};

pub type ResolveFallbackTokens<'a> = dyn Fn(&[InlineToken], usize, usize) -> Vec<InlineToken> + 'a;

pub struct ProcessorHookGroups<'a> {
    pub(crate) tokenizers: &'a [Box<dyn yozora_core_tokenizer::InlineTokenizer>],
    pub(crate) match_inline_api: &'a dyn MatchInlineFallbackPhaseApi,
    pub(crate) resolve_fallback_tokens: &'a ResolveFallbackTokens<'a>,
}

pub struct PhrasingContentProcessor<'a> {
    pub(crate) hook_groups: Option<ProcessorHookGroups<'a>>,
    pub(crate) hook_group_index: usize,
    pub(crate) hooks: Vec<MatchInlineProcessorHook<'a>>,
}

impl PhrasingContentProcessor<'_> {
    pub fn process(
        &mut self,
        higher_priority_tokens: &[InlineToken],
        start_index: usize,
        end_index: usize,
    ) -> Vec<InlineToken> {
        if let Some(hook_groups) = &self.hook_groups {
            super::process_tokenizer_groups(
                hook_groups,
                self.hook_group_index,
                higher_priority_tokens,
                start_index,
                end_index,
            )
        } else {
            super::match_inline_tokens(
                &mut self.hooks,
                higher_priority_tokens,
                start_index,
                end_index,
            )
        }
    }
}

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
