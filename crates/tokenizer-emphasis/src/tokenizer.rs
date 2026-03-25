use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const EMPHASIS_TOKENIZER_NAME: &str = "@yozora/tokenizer-emphasis";

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

struct EmphasisMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> MatchInlineHook<'a> for EmphasisMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let api = self.api;
        Box::new(genFindDelimiter(move |start_index, end_index| {
            r#match::find_delimiter(api, start_index, end_index)
        }))
    }

    fn isDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        r#match::is_delimiter_pair(self.api, opener_delimiter, closer_delimiter)
    }

    fn processDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
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

struct EmphasisParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for EmphasisParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_emphasis_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for EmphasisTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(EmphasisMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(EmphasisParseHook { api })
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
        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn getNodePoints(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn getBlockStartIndex(&self) -> usize {
            0
        }

        fn getBlockEndIndex(&self) -> usize {
            self.node_points.len()
        }

        fn resolveFallbackTokens(
            &self,
            _tokens: &[InlineToken],
            _token_start_index: usize,
            _token_end_index: usize,
        ) -> Vec<InlineToken> {
            Vec::new()
        }

        fn resolveInternalTokens(
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
        fn shouldReservePosition(&self) -> bool {
            false
        }

        fn calcPosition(&self, _interval: NodeInterval) -> yozora_ast::Position {
            panic!("calcPosition should not be called in this test")
        }

        fn formatUrl(&self, url: &str) -> String {
            url.to_string()
        }

        fn getNodePoints(&self) -> &[NodePoint] {
            &[]
        }

        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn parseInlineTokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node> {
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
        let mut find_delimiter = hook.findDelimiter();
        let delimiter = find_delimiter
            .next((0, api.getBlockEndIndex()))
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
        let result = hook.processDelimiterPair(
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
            .data_as::<parse::EmphasisTokenData>()
            .expect("expected emphasis token data");
        assert_eq!(data.thickness, 2);
        assert_eq!(data.children.len(), resolved_tokens.len());
        assert_eq!(data.children[0].tokenizer, resolved_tokens[0].tokenizer);
        assert_eq!(data.children[0].node_type, resolved_tokens[0].node_type);
        assert_eq!(data.children[0].start_index, resolved_tokens[0].start_index);
        assert_eq!(data.children[0].end_index, resolved_tokens[0].end_index);
        assert!(result.remainOpenerDelimiter.is_none());
        assert!(result.remainCloserDelimiter.is_none());
    }

    #[test]
    fn engine_parse_should_parse_children_tokens() {
        let tokenizer = EmphasisTokenizer::default();
        let api = DummyParseApi;
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(EMPHASIS_TOKENIZER_NAME, EMPHASIS_TYPE, (1, 4)).with_data(
            parse::EmphasisTokenData {
                thickness: 1,
                children: vec![InlineToken::new("text", TEXT_TYPE, (2, 3))],
            },
        );

        let nodes = parse_hook.parse(&[token]);
        assert_eq!(nodes.len(), 1);

        let Node::Emphasis(node) = &nodes[0] else {
            panic!("expected emphasis node")
        };
        assert_eq!(node.children.len(), 1);
        assert!(matches!(node.children[0], Node::Text(_)));
    }
}
