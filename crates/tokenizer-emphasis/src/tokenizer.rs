use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use crate::types::EmphasisTokenData;
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{EmphasisDelimiter, EMPHASIS_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct EmphasisTokenizer {
    meta: TokenizerMeta,
}

impl Default for EmphasisTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl EmphasisTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| EMPHASIS_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options
                    .priority
                    .unwrap_or(TokenizerPriority::CONTAINING_INLINE),
            },
        }
    }
}

impl Tokenizer for EmphasisTokenizer {
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

pub struct EmphasisDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl EmphasisDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<EmphasisDelimiter> {
        r#match::find_delimiter(self.api, range_index.0, range_index.1)
    }
}

pub struct EmphasisMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> EmphasisMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self { api }
    }

    pub fn find_delimiter(&self) -> EmphasisDelimiterGenerator<'a> {
        EmphasisDelimiterGenerator { api: self.api }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &EmphasisDelimiter,
        closer_delimiter: &EmphasisDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        r#match::is_delimiter_pair(self.api, opener_delimiter, closer_delimiter)
    }

    pub fn process_delimiter_pair(
        &self,
        opener_delimiter: &EmphasisDelimiter,
        closer_delimiter: &EmphasisDelimiter,
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

impl<'a> MatchInlineHook<'a> for EmphasisMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let mut finder = EmphasisMatchHook::find_delimiter(self);
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
        EmphasisMatchHook::is_delimiter_pair(
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
        EmphasisMatchHook::process_delimiter_pair(
            self,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }
}

pub struct EmphasisParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> EmphasisParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for EmphasisParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_emphasis_tokens(tokens, self.api).resolve(self.api)
    }
}

impl InlineTokenizer for EmphasisTokenizer {
    fn parse_deferred<'a>(
        &'a self,
        tokens: &'a [InlineToken],
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Option<ParseInlineHookResult<'a>> {
        Some(parse::parse_emphasis_tokens(tokens, api))
    }

    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(EmphasisMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(EmphasisParseHook::new(api))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::{Node, Text, EMPHASIS_TYPE, STRONG_TYPE, TEXT_TYPE};
    use yozora_character::{create_node_point_generator, NodePoint};

    struct DummyMatchApi {
        node_points: Vec<NodePoint>,
        resolved_tokens: Vec<InlineToken>,
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
            self.resolved_tokens.clone()
        }
    }

    struct DummyParseApi;

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
            &[]
        }

        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn parse_inline_tokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node> {
            let Some(tokens) = tokens else {
                return Vec::new();
            };
            tokens
                .iter()
                .map(|_| {
                    Node::Text(Text {
                        position: None,
                        value: "x".to_string(),
                    })
                })
                .collect()
        }
    }

    #[test]
    fn engine_match_should_find_underscore_delimiter() {
        let tokenizer = EmphasisTokenizer::default();
        let node_points = create_node_point_generator("__foo__")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi {
            node_points,
            resolved_tokens: Vec::new(),
        };

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, api.get_block_end_index()))
            .expect("expected delimiter");

        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 2);
        assert_eq!(delimiter.thickness, 2);
        assert_eq!(delimiter.delimiter_type, DelimiterType::Opener);
    }

    #[test]
    fn engine_process_pair_should_build_strong_token() {
        let tokenizer = EmphasisTokenizer::default();
        let node_points = create_node_point_generator("**foo**")
            .pop()
            .expect("expected node points");
        let resolved_tokens = vec![InlineToken::new("text", TEXT_TYPE, (2, 5))];
        let api = DummyMatchApi {
            node_points,
            resolved_tokens: resolved_tokens.clone(),
        };

        let hook = tokenizer.r#match(&api);
        let result = hook.process_delimiter_pair(
            &TokenDelimiter {
                delimiter_type: DelimiterType::Opener,
                start_index: 0,
                end_index: 2,
                thickness: 2,
                original_thickness: 2,
            },
            &TokenDelimiter {
                delimiter_type: DelimiterType::Closer,
                start_index: 5,
                end_index: 7,
                thickness: 2,
                original_thickness: 2,
            },
            &[],
        );

        assert_eq!(result.tokens.len(), 1);
        let token = &result.tokens[0];
        assert_eq!(token.node_type, STRONG_TYPE);
        assert_eq!(token.start_index, 0);
        assert_eq!(token.end_index, 7);
        let data = token
            .data_as::<EmphasisTokenData>()
            .expect("expected emphasis token data");
        assert_eq!(data.thickness, 2);
        assert_eq!(token.children.len(), resolved_tokens.len());
        assert_eq!(token.children[0].tokenizer, resolved_tokens[0].tokenizer);
        assert_eq!(token.children[0].node_type, resolved_tokens[0].node_type);
        assert_eq!(
            token.children[0].start_index,
            resolved_tokens[0].start_index
        );
        assert_eq!(token.children[0].end_index, resolved_tokens[0].end_index);
        assert!(result.remain_opener_delimiter.is_none());
        assert!(result.remain_closer_delimiter.is_none());
    }

    #[test]
    fn engine_parse_should_parse_children_tokens() {
        let tokenizer = EmphasisTokenizer::default();
        let api = DummyParseApi;
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(EMPHASIS_TOKENIZER_NAME, EMPHASIS_TYPE, (1, 4))
            .with_children(vec![InlineToken::new("text", TEXT_TYPE, (2, 3))])
            .with_data(EmphasisTokenData { thickness: 1 });

        let nodes = parse_hook.parse(&[token]);
        assert_eq!(nodes.len(), 1);

        let Node::Emphasis(node) = &nodes[0] else {
            panic!("expected emphasis node")
        };
        assert_eq!(node.children.len(), 1);
        assert!(matches!(node.children[0], Node::Text(_)));
    }
}
