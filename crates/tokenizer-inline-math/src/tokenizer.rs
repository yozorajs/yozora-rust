use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use crate::types::InlineMathTokenData;
#[cfg(test)]
use yozora_ast::INLINE_MATH_TYPE;
#[cfg(test)]
use yozora_character::NodePoint;
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{
    InlineMathDelimiter, InlineMathTokenizerOptions, INLINE_MATH_TOKENIZER_NAME,
    INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME,
};
use crate::{match_with_backtick as backtick_match, parse, r#match};

#[derive(Debug, Clone)]
pub struct InlineMathTokenizer {
    meta: TokenizerMeta,
    backtick_required: bool,
}

impl Default for InlineMathTokenizer {
    fn default() -> Self {
        Self::new(InlineMathTokenizerOptions::default())
    }
}

impl InlineMathTokenizer {
    pub fn new(options: InlineMathTokenizerOptions) -> Self {
        let (default_name, default_priority) = if options.backtick_required {
            (
                INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME,
                TokenizerPriority::ATOMIC,
            )
        } else {
            (
                INLINE_MATH_TOKENIZER_NAME,
                TokenizerPriority::INTERRUPTABLE_INLINE,
            )
        };

        Self {
            meta: TokenizerMeta {
                name: options.name.unwrap_or_else(|| default_name.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(default_priority),
            },
            backtick_required: options.backtick_required,
        }
    }
}

impl Tokenizer for InlineMathTokenizer {
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

pub struct InlineMathBacktickDelimiterGenerator {
    finder: backtick_match::InlineMathBacktickDelimiterFinder,
}

impl InlineMathBacktickDelimiterGenerator {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<InlineMathDelimiter> {
        self.finder.find_next_delimiter(range_index.0)
    }
}

pub struct InlineMathPlainDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl InlineMathPlainDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<InlineMathDelimiter> {
        r#match::find_delimiter(self.api, range_index.0, range_index.1)
    }
}

pub enum InlineMathDelimiterGenerator<'a> {
    Backtick(InlineMathBacktickDelimiterGenerator),
    Plain(InlineMathPlainDelimiterGenerator<'a>),
}

impl InlineMathDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<InlineMathDelimiter> {
        match self {
            Self::Backtick(finder) => finder.next(range_index),
            Self::Plain(finder) => finder.next(range_index),
        }
    }
}

pub struct InlineMathBacktickMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> InlineMathBacktickMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self { api }
    }

    pub fn find_delimiter(&self) -> InlineMathBacktickDelimiterGenerator {
        InlineMathBacktickDelimiterGenerator {
            finder: backtick_match::InlineMathBacktickDelimiterFinder::new(self.api),
        }
    }

    pub fn process_single_delimiter(&self, delimiter: &InlineMathDelimiter) -> Vec<InlineToken> {
        backtick_match::process_single_delimiter(delimiter)
    }
}

impl<'a> MatchInlineHook<'a> for InlineMathBacktickMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let mut finder = InlineMathBacktickMatchHook::find_delimiter(self);
        Box::new(gen_find_delimiter(move |start_index, _end_index| {
            finder.next((start_index, _end_index))
        }))
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        InlineMathBacktickMatchHook::process_single_delimiter(self, delimiter)
    }
}

pub struct InlineMathPlainMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> InlineMathPlainMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self { api }
    }

    pub fn find_delimiter(&self) -> InlineMathPlainDelimiterGenerator<'a> {
        InlineMathPlainDelimiterGenerator { api: self.api }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &InlineMathDelimiter,
        closer_delimiter: &InlineMathDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        r#match::is_delimiter_pair(opener_delimiter, closer_delimiter)
    }

    pub fn process_delimiter_pair(
        &self,
        opener_delimiter: &InlineMathDelimiter,
        closer_delimiter: &InlineMathDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        r#match::process_delimiter_pair(opener_delimiter, closer_delimiter)
    }
}

impl<'a> MatchInlineHook<'a> for InlineMathPlainMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let mut finder = InlineMathPlainMatchHook::find_delimiter(self);
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
        InlineMathPlainMatchHook::is_delimiter_pair(
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
        InlineMathPlainMatchHook::process_delimiter_pair(
            self,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }
}

pub enum InlineMathMatchHook<'a> {
    Backtick(InlineMathBacktickMatchHook<'a>),
    Plain(InlineMathPlainMatchHook<'a>),
}

impl<'a> InlineMathMatchHook<'a> {
    pub fn new(tokenizer: &InlineMathTokenizer, api: &'a dyn MatchInlinePhaseApi) -> Self {
        if tokenizer.backtick_required {
            Self::Backtick(InlineMathBacktickMatchHook::new(api))
        } else {
            Self::Plain(InlineMathPlainMatchHook::new(api))
        }
    }

    pub fn find_delimiter(&self) -> InlineMathDelimiterGenerator<'a> {
        match self {
            Self::Backtick(hook) => InlineMathDelimiterGenerator::Backtick(hook.find_delimiter()),
            Self::Plain(hook) => InlineMathDelimiterGenerator::Plain(hook.find_delimiter()),
        }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &InlineMathDelimiter,
        closer_delimiter: &InlineMathDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        match self {
            Self::Backtick(hook) => MatchInlineHook::is_delimiter_pair(
                hook,
                opener_delimiter,
                closer_delimiter,
                internal_tokens,
            ),
            Self::Plain(hook) => {
                hook.is_delimiter_pair(opener_delimiter, closer_delimiter, internal_tokens)
            }
        }
    }

    pub fn process_delimiter_pair(
        &self,
        opener_delimiter: &InlineMathDelimiter,
        closer_delimiter: &InlineMathDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        match self {
            Self::Backtick(hook) => MatchInlineHook::process_delimiter_pair(
                hook,
                opener_delimiter,
                closer_delimiter,
                internal_tokens,
            ),
            Self::Plain(hook) => {
                hook.process_delimiter_pair(opener_delimiter, closer_delimiter, internal_tokens)
            }
        }
    }

    pub fn process_single_delimiter(&self, delimiter: &InlineMathDelimiter) -> Vec<InlineToken> {
        match self {
            Self::Backtick(hook) => hook.process_single_delimiter(delimiter),
            Self::Plain(_) => Vec::new(),
        }
    }
}

impl<'a> MatchInlineHook<'a> for InlineMathMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        match self {
            Self::Backtick(hook) => MatchInlineHook::find_delimiter(hook),
            Self::Plain(hook) => MatchInlineHook::find_delimiter(hook),
        }
    }

    fn is_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        InlineMathMatchHook::is_delimiter_pair(
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
        InlineMathMatchHook::process_delimiter_pair(
            self,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        InlineMathMatchHook::process_single_delimiter(self, delimiter)
    }
}

pub struct InlineMathParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> InlineMathParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for InlineMathParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_inline_math_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for InlineMathTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(InlineMathMatchHook::new(self, api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(InlineMathParseHook::new(api))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::Node;
    use yozora_character::create_node_point_generator;

    struct DummyMatchApi {
        node_points: Vec<NodePoint>,
    }

    impl MatchInlinePhaseApi for DummyMatchApi {
        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn get_node_points(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn get_block_start_index(&self) -> usize {
            0
        }

        fn get_block_end_index(&self) -> usize {
            self.node_points.len()
        }

        fn resolve_fallback_tokens(
            &self,
            _tokens: &[InlineToken],
            _token_start_index: usize,
            _token_end_index: usize,
        ) -> Vec<InlineToken> {
            Vec::new()
        }

        fn resolve_internal_tokens(
            &self,
            _higher_priority_tokens: &[InlineToken],
            _start_index: usize,
            _end_index: usize,
        ) -> Vec<InlineToken> {
            Vec::new()
        }
    }

    struct DummyParseApi {
        node_points: Vec<NodePoint>,
    }

    impl ParseInlinePhaseApi for DummyParseApi {
        fn should_reserve_position(&self) -> bool {
            false
        }

        fn calc_position(&self, _interval: NodeInterval) -> yozora_ast::Position {
            panic!("calc_position should not be called in this test")
        }

        fn format_url(&self, url: &str) -> String {
            url.to_string()
        }

        fn get_node_points(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn parse_inline_tokens(&self, _tokens: Option<&[InlineToken]>) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn engine_match_should_find_inline_math_opener() {
        let tokenizer = InlineMathTokenizer::new(InlineMathTokenizerOptions {
            backtick_required: false,
            ..InlineMathTokenizerOptions::default()
        });
        let node_points = create_node_point_generator("$x$")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, api.get_block_end_index()))
            .expect("expected inline math delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Opener);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 1);
    }

    #[test]
    fn engine_parse_should_build_inline_math_node() {
        let tokenizer = InlineMathTokenizer::new(InlineMathTokenizerOptions {
            backtick_required: false,
            ..InlineMathTokenizerOptions::default()
        });
        let node_points = create_node_point_generator("$x$")
            .pop()
            .expect("expected node points");
        let api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(INLINE_MATH_TOKENIZER_NAME, INLINE_MATH_TYPE, (0, 3))
            .with_data(InlineMathTokenData { thickness: 1 });
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        let Node::InlineMath(node) = &nodes[0] else {
            panic!("expected inline math node");
        };
        assert_eq!(node.value, "x");
    }
}
