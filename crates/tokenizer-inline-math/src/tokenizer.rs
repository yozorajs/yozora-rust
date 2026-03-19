use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_ast::INLINE_MATH_TYPE;
#[cfg(test)]
use yozora_character::NodePoint;
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const INLINE_MATH_TOKENIZER_NAME: &str = "@yozora/tokenizer-inline-math";
pub const INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME: &str =
    "@yozora/tokenizer-inline-math_with_backtick";

#[derive(Debug, Clone, Copy)]
pub struct InlineMathTokenizerOptions {
    pub backtick_required: bool,
}

impl Default for InlineMathTokenizerOptions {
    fn default() -> Self {
        Self {
            backtick_required: true,
        }
    }
}

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
        let (name, priority) = if options.backtick_required {
            (INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME, 10)
        } else {
            (INLINE_MATH_TOKENIZER_NAME, 2)
        };

        Self {
            meta: TokenizerMeta {
                name: name.to_string(),
                kind: TokenizerKind::Inline,
                priority,
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

struct InlineMathBacktickMatchHook {
    delimiter_finder: r#match::InlineMathBacktickDelimiterFinder,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for InlineMathBacktickMatchHook {
    fn reset(&mut self) {
        self.delimiter_finder.reset();
        self.last_end_index = None;
        self.last_delimiter = None;
    }

    fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        let mut last_end_index = self.last_end_index;
        let mut last_delimiter = self.last_delimiter.clone();
        let delimiter = genFindDelimiter(
            range_index,
            &mut last_end_index,
            &mut last_delimiter,
            |start_index, _end_index| self.delimiter_finder.find_next_delimiter(start_index),
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        r#match::process_single_delimiter(delimiter)
    }
}

struct InlineMathPlainMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for InlineMathPlainMatchHook<'_> {
    fn reset(&mut self) {
        self.last_end_index = None;
        self.last_delimiter = None;
    }

    fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        let mut last_end_index = self.last_end_index;
        let mut last_delimiter = self.last_delimiter.clone();
        let delimiter = genFindDelimiter(
            range_index,
            &mut last_end_index,
            &mut last_delimiter,
            |start_index, end_index| r#match::find_delimiter(self.api, start_index, end_index),
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
    }

    fn isDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        r#match::is_delimiter_pair(opener_delimiter, closer_delimiter)
    }

    fn processDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        r#match::process_delimiter_pair(opener_delimiter, closer_delimiter)
    }
}

struct InlineMathParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for InlineMathParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_inline_math_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for InlineMathTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook + 'a> {
        if self.backtick_required {
            Box::new(InlineMathBacktickMatchHook {
                delimiter_finder: r#match::InlineMathBacktickDelimiterFinder::new(api),
                last_end_index: None,
                last_delimiter: None,
            })
        } else {
            Box::new(InlineMathPlainMatchHook {
                api,
                last_end_index: None,
                last_delimiter: None,
            })
        }
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(InlineMathParseHook { api })
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

        fn calc_position(&self, _interval: NodeInterval) -> Option<yozora_ast::Position> {
            None
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

        fn parse_inline_tokens(&self, _tokens: &[InlineToken]) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn engine_match_should_find_inline_math_opener() {
        let tokenizer = InlineMathTokenizer::new(InlineMathTokenizerOptions {
            backtick_required: false,
        });
        let node_points = create_node_point_generator("$x$")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let mut hook = tokenizer.r#match(&api);
        let delimiter = hook
            .findDelimiter((0, api.get_block_end_index()))
            .expect("expected inline math delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Opener);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 1);
    }

    #[test]
    fn engine_parse_should_build_inline_math_node() {
        let tokenizer = InlineMathTokenizer::new(InlineMathTokenizerOptions {
            backtick_required: false,
        });
        let node_points = create_node_point_generator("$x$")
            .pop()
            .expect("expected node points");
        let api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(INLINE_MATH_TOKENIZER_NAME, INLINE_MATH_TYPE, (0, 3))
            .with_data(parse::InlineMathTokenData { thickness: 1 });
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        let Node::InlineMath(node) = &nodes[0] else {
            panic!("expected inline math node");
        };
        assert_eq!(node.value, "x");
    }
}
