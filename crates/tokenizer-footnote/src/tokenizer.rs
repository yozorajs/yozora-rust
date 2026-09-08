use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::types::{FootnoteDelimiter, FOOTNOTE_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct FootnoteTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl FootnoteTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| FOOTNOTE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::LINKS),
            },
        }
    }
}

impl Tokenizer for FootnoteTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Inline
    }

    fn name(&self) -> &str {
        &self.meta.name
    }

    fn priority(&self) -> i32 {
        self.meta.priority
    }
}

pub struct FootnoteDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl FootnoteDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<FootnoteDelimiter> {
        r#match::find_delimiter_entry(self.api.get_node_points(), range_index.0, range_index.1)
    }
}

pub struct FootnoteMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> FootnoteMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self { api }
    }

    pub fn find_delimiter(&self) -> FootnoteDelimiterGenerator<'a> {
        FootnoteDelimiterGenerator { api: self.api }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &FootnoteDelimiter,
        closer_delimiter: &FootnoteDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        r#match::is_delimiter_pair(
            self.api,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }

    pub fn process_delimiter_pair(
        &self,
        opener_delimiter: &FootnoteDelimiter,
        closer_delimiter: &FootnoteDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        r#match::process_delimiter_pair(
            self.api,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }
}

impl<'a> MatchInlineHook<'a> for FootnoteMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let mut finder = FootnoteMatchHook::find_delimiter(self);

        Box::new(gen_find_delimiter(move |start_index, end_index| {
            finder.next((start_index, end_index))
        }))
    }

    fn is_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        FootnoteMatchHook::is_delimiter_pair(
            self,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }

    fn process_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        FootnoteMatchHook::process_delimiter_pair(
            self,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }
}

pub struct FootnoteParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> FootnoteParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for FootnoteParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_footnote_tokens(tokens, self.api).resolve(self.api)
    }
}

impl InlineTokenizer for FootnoteTokenizer {
    fn parse_deferred<'a>(
        &'a self,
        tokens: &'a [InlineToken],
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Option<ParseInlineHookResult<'a>> {
        Some(parse::parse_footnote_tokens(tokens, api))
    }

    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(FootnoteMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(FootnoteParseHook::new(api))
    }
}
