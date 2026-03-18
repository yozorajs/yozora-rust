use yozora_ast::{DeleteNode, Node, DELETE_TYPE};
use yozora_character::{is_whitespace_character, AsciiCodePoint};
use yozora_core_tokenizer::engine::{
    DelimiterType, EngineInlineTokenizer, EngineTokenizer, InlineToken, MatchInlineHook,
    MatchInlinePhaseApi as EngineMatchInlinePhaseApi, ParseInlineHook,
    ParseInlinePhaseApi as EngineParseInlinePhaseApi, ProcessDelimiterPairResult, TokenDelimiter,
    TokenizerType,
};
use yozora_core_tokenizer::phase::NodeInterval;
use yozora_core_tokenizer::{
    InlineTokenizer, MatchInlinePhaseApi, ParseInlinePhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

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
                priority: 3,
            },
        }
    }
}

impl Tokenizer for DeleteTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for DeleteTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_delete_tokens(input)?;
        Some(parse::parse_delete_tokens(input, &tokens))
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        position: Option<yozora_ast::Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, position)
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        position: Option<yozora_ast::Position>,
        _match_api: &dyn MatchInlinePhaseApi,
        _parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, position)
    }
}

#[derive(Debug, Clone)]
struct DeleteTokenData {
    children: Vec<InlineToken>,
}

impl EngineTokenizer for DeleteTokenizer {
    fn tokenizer_type(&self) -> TokenizerType {
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
    api: &'a dyn EngineMatchInlinePhaseApi,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl DeleteMatchHook<'_> {
    fn find_delimiter_impl(&self, start_index: usize, end_index: usize) -> Option<TokenDelimiter> {
        let node_points = self.api.get_node_points();
        if start_index >= end_index || end_index > node_points.len() {
            return None;
        }

        let mut i = start_index;
        while i < end_index {
            let c = node_points[i].code_point;
            if c == AsciiCodePoint::BACKSLASH as i32 {
                i += 2;
                continue;
            }

            if c != AsciiCodePoint::TILDE as i32 {
                i += 1;
                continue;
            }

            let start = i;
            i += 1;
            while i < end_index && node_points[i].code_point == c {
                i += 1;
            }

            let end = i;
            if end.saturating_sub(start) != 2 {
                continue;
            }

            let mut delimiter_type = DelimiterType::Both;

            let preceding = if start == start_index {
                None
            } else {
                node_points.get(start - 1)
            };
            if preceding.is_some_and(|p| is_whitespace_character(p.code_point)) {
                delimiter_type = DelimiterType::Opener;
            }

            let following = if end == end_index {
                None
            } else {
                node_points.get(end)
            };
            if following.is_some_and(|p| is_whitespace_character(p.code_point)) {
                if delimiter_type != DelimiterType::Both {
                    continue;
                }
                delimiter_type = DelimiterType::Closer;
            }

            return Some(TokenDelimiter {
                delimiter_type,
                start_index: start,
                end_index: end,
                thickness: 2,
                original_thickness: 2,
            });
        }

        None
    }
}

impl MatchInlineHook for DeleteMatchHook<'_> {
    fn reset(&mut self) {
        self.last_end_index = None;
        self.last_delimiter = None;
    }

    fn find_delimiter(&mut self, start_index: usize, end_index: usize) -> Option<TokenDelimiter> {
        if self.last_end_index == Some(end_index) {
            match &self.last_delimiter {
                Some(delimiter) if delimiter.start_index >= start_index => {
                    return Some(delimiter.clone());
                }
                None => return None,
                _ => {}
            }
        }

        self.last_end_index = Some(end_index);
        self.last_delimiter = self.find_delimiter_impl(start_index, end_index);
        self.last_delimiter.clone()
    }

    fn process_delimiter_pair(
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
            tokens: vec![InlineToken::new(
                "",
                DELETE_TYPE,
                (opener_delimiter.start_index, closer_delimiter.end_index),
            )
            .with_data(DeleteTokenData { children })],
            remain_opener_delimiter: None,
            remain_closer_delimiter: None,
        }
    }
}

struct DeleteParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for DeleteParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<DeleteTokenData>() else {
                continue;
            };

            let children = self.api.parse_inline_tokens(&data.children);
            let position = if self.api.should_reserve_position() {
                self.api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                })
            } else {
                None
            };

            nodes.push(Node::Delete(DeleteNode { position, children }));
        }

        nodes
    }
}

impl EngineInlineTokenizer for DeleteTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(DeleteMatchHook {
            api,
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(DeleteParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::TEXT_TYPE;
    use yozora_character::{create_node_point_generator, NodePoint};

    struct DummyMatchApi {
        node_points: Vec<NodePoint>,
        resolved_tokens: Vec<InlineToken>,
    }

    impl EngineMatchInlinePhaseApi for DummyMatchApi {
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

    impl EngineParseInlinePhaseApi for DummyParseApi {
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

        let mut hook = tokenizer.create_match_hook(&api);
        let delimiter = hook
            .find_delimiter(0, api.get_block_end_index())
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Both);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 2);
    }

    #[test]
    fn engine_parse_should_parse_delete_children() {
        let tokenizer = DeleteTokenizer::default();
        let api = DummyParseApi;
        let parse_hook = tokenizer.create_parse_hook(&api);

        let token = InlineToken::new(DELETE_TOKENIZER_NAME, DELETE_TYPE, (0, 6)).with_data(
            DeleteTokenData {
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
