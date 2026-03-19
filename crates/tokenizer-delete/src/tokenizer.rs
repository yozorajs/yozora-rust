use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const DELETE_TOKENIZER_NAME: &str = "@yozora/tokenizer-delete";

#[derive(Debug, Clone)]
pub struct DeleteTokenizer {
    meta: TokenizerMeta,
}

impl Default for DeleteTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: DELETE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 2,
            },
        }
    }
}

impl Tokenizer for DeleteTokenizer {
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

struct DeleteMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for DeleteMatchHook<'_> {
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
            |start_index, end_index| {
                r#match::find_delete_delimiter(
                    self.api.get_node_points(),
                    start_index,
                    end_index,
                )
            },
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
    }

    fn processDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let children = self.api.resolve_internal_tokens(
            internal_tokens,
            opener_delimiter.end_index,
            closer_delimiter.start_index,
        );

        ProcessDelimiterPairResult {
            tokens: vec![r#match::create_delete_token(
                opener_delimiter,
                closer_delimiter,
                children,
            )],
            remainOpenerDelimiter: None,
            remainCloserDelimiter: None,
        }
    }
}

struct DeleteParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for DeleteParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_delete_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for DeleteTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(DeleteMatchHook {
            api,
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn parse<'a>(
        &'a self,
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(DeleteParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use yozora_ast::{Node, DELETE_TYPE, TEXT_TYPE};
    use yozora_character::{create_node_point_generator, NodePoint};

    use super::*;

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

        fn calc_position(&self, _interval: NodeInterval) -> Option<yozora_ast::Position> {
            None
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

        fn parse_inline_tokens(&self, tokens: &[InlineToken]) -> Vec<Node> {
            tokens
                .iter()
                .map(|_| {
                    Node::Text(yozora_ast::Text {
                        position: None,
                        value: "x".to_string(),
                    })
                })
                .collect()
        }
    }

    #[test]
    fn engine_match_should_find_tilde_delimiter() {
        let tokenizer = DeleteTokenizer::default();
        let node_points = create_node_point_generator("~~foo~~")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi {
            node_points,
            resolved_tokens: Vec::new(),
        };

        let mut hook = tokenizer.r#match(&api);
        let delimiter = hook
            .findDelimiter((0, api.get_block_end_index()))
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Both);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 2);
    }

    #[test]
    fn engine_parse_should_parse_delete_children() {
        let tokenizer = DeleteTokenizer::default();
        let api = DummyParseApi;
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(DELETE_TOKENIZER_NAME, DELETE_TYPE, (0, 6)).with_data(
            parse::DeleteTokenData {
                children: vec![InlineToken::new("text", TEXT_TYPE, (2, 4))],
            },
        );

        let nodes = parse_hook.parse(&[token]);
        assert_eq!(nodes.len(), 1);

        let Node::Delete(node) = &nodes[0] else {
            panic!("expected delete node");
        };
        assert_eq!(node.children.len(), 1);
        assert!(matches!(node.children[0], Node::Text(_)));
    }
}
