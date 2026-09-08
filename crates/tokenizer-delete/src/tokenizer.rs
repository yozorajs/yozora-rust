use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use crate::types::DeleteTokenData;
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{DeleteDelimiter, DELETE_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct DeleteTokenizer {
    meta: TokenizerMeta,
}

impl Default for DeleteTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl DeleteTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| DELETE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options
                    .priority
                    .unwrap_or(TokenizerPriority::CONTAINING_INLINE),
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

pub struct DeleteDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl DeleteDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<DeleteDelimiter> {
        r#match::find_delete_delimiter(self.api.get_node_points(), range_index.0, range_index.1)
    }
}

pub struct DeleteMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> DeleteMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self { api }
    }

    pub fn find_delimiter(&self) -> DeleteDelimiterGenerator<'a> {
        DeleteDelimiterGenerator { api: self.api }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &DeleteDelimiter,
        closer_delimiter: &DeleteDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        if opener_delimiter.thickness == closer_delimiter.thickness {
            IsDelimiterPairResult::Paired
        } else {
            IsDelimiterPairResult::NotPaired {
                opener: true,
                closer: true,
            }
        }
    }

    pub fn process_delimiter_pair(
        &self,
        opener_delimiter: &DeleteDelimiter,
        closer_delimiter: &DeleteDelimiter,
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
            remain_opener_delimiter: None,
            remain_closer_delimiter: None,
        }
    }
}

impl<'a> MatchInlineHook<'a> for DeleteMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let mut finder = DeleteMatchHook::find_delimiter(self);

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
        DeleteMatchHook::is_delimiter_pair(
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
        DeleteMatchHook::process_delimiter_pair(
            self,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }
}

pub struct DeleteParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> DeleteParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for DeleteParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_delete_tokens(tokens, self.api).resolve(self.api)
    }
}

impl InlineTokenizer for DeleteTokenizer {
    fn parse_deferred<'a>(
        &'a self,
        tokens: &'a [InlineToken],
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Option<ParseInlineHookResult<'a>> {
        Some(parse::parse_delete_tokens(tokens, api))
    }

    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(DeleteMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(DeleteParseHook::new(api))
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

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, api.get_block_end_index()))
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Both);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 2);
    }

    #[test]
    fn engine_match_should_find_single_tilde_delimiter() {
        let tokenizer = DeleteTokenizer::default();
        let node_points = create_node_point_generator("~foo~")
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

        assert_eq!(delimiter.thickness, 1);
    }

    #[test]
    fn engine_parse_should_parse_delete_children() {
        let tokenizer = DeleteTokenizer::default();
        let api = DummyParseApi;
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(DELETE_TOKENIZER_NAME, DELETE_TYPE, (0, 6))
            .with_children(vec![InlineToken::new("text", TEXT_TYPE, (2, 4))])
            .with_data(DeleteTokenData);

        let nodes = parse_hook.parse(&[token]);
        assert_eq!(nodes.len(), 1);

        let Node::Delete(node) = &nodes[0] else {
            panic!("expected delete node");
        };
        assert_eq!(node.children.len(), 1);
        assert!(matches!(node.children[0], Node::Text(_)));
    }
}
