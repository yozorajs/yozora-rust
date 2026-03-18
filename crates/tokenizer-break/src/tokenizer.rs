use yozora_ast::{BreakNode, Node, BREAK_TYPE};
use yozora_character::{AsciiCodePoint, VirtualCodePoint};
use yozora_core_tokenizer::engine::{
    DelimiterType, EngineInlineTokenizer, EngineTokenizer, InlineToken, MatchInlineHook,
    MatchInlinePhaseApi as EngineMatchInlinePhaseApi, ParseInlineHook,
    ParseInlinePhaseApi as EngineParseInlinePhaseApi, TokenDelimiter, TokenizerType,
};
use yozora_core_tokenizer::phase::NodeInterval;
use yozora_core_tokenizer::{
    InlineTokenizer, MatchInlinePhaseApi, ParseInlinePhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-break";

#[derive(Debug, Clone)]
pub struct BreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for BreakTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: BREAK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 1,
            },
        }
    }
}

impl Tokenizer for BreakTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for BreakTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_break_tokens(input)?;
        Some(parse::parse_break_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_break_tokens(input)?;
        Some(parse::parse_break_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        _match_api: &dyn MatchInlinePhaseApi,
        parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_break_tokens(input)?;
        Some(parse::parse_break_tokens(input, &tokens, Some(parse_api)))
    }
}

impl EngineTokenizer for BreakTokenizer {
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

struct BreakMatchHook<'a> {
    api: &'a dyn EngineMatchInlinePhaseApi,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl BreakMatchHook<'_> {
    fn find_delimiter_impl(&self, start_index: usize, end_index: usize) -> Option<TokenDelimiter> {
        let node_points = self.api.get_node_points();
        if start_index + 1 >= end_index || end_index > node_points.len() {
            return None;
        }

        for i in (start_index + 1)..end_index {
            if node_points[i].code_point != VirtualCodePoint::LineEnd as i32 {
                continue;
            }

            let prev = node_points[i - 1].code_point;
            let marker_start = if prev == AsciiCodePoint::BACKSLASH as i32 {
                let mut x = i.saturating_sub(2) as isize;
                while x >= start_index as isize
                    && node_points[x as usize].code_point == AsciiCodePoint::BACKSLASH as i32
                {
                    x -= 1;
                }

                if ((i as isize - x) & 1) == 0 {
                    Some(i - 1)
                } else {
                    None
                }
            } else if prev == AsciiCodePoint::SPACE as i32 {
                let mut x = i.saturating_sub(2) as isize;
                while x >= start_index as isize
                    && node_points[x as usize].code_point == AsciiCodePoint::SPACE as i32
                {
                    x -= 1;
                }

                if i as isize - x > 2 {
                    Some((x + 1) as usize)
                } else {
                    None
                }
            } else {
                None
            };

            let Some(marker_start) = marker_start else {
                continue;
            };

            return Some(TokenDelimiter {
                delimiter_type: DelimiterType::Full,
                start_index: marker_start,
                end_index: i,
                thickness: i.saturating_sub(marker_start),
                original_thickness: i.saturating_sub(marker_start),
            });
        }

        None
    }
}

impl MatchInlineHook for BreakMatchHook<'_> {
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

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        vec![InlineToken::new(
            "",
            BREAK_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )]
    }
}

struct BreakParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for BreakParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());
        for token in tokens {
            let position = if self.api.should_reserve_position() {
                self.api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                })
            } else {
                None
            };

            nodes.push(Node::Break(BreakNode { position }));
        }
        nodes
    }
}

impl EngineInlineTokenizer for BreakTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(BreakMatchHook {
            api,
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(BreakParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::{create_node_point_generator, NodePoint};

    struct DummyInlineApi;

    impl MatchInlinePhaseApi for DummyInlineApi {
        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }
    }

    #[test]
    fn phase_api_should_preserve_break_result() {
        let tokenizer = BreakTokenizer::default();
        let api = DummyInlineApi;

        let nodes = tokenizer
            .tokenize_inline_with_api("line  \nnext", None, &api)
            .expect("should parse hard break");

        assert!(matches!(nodes.get(1), Some(Node::Break(_))));
    }

    struct DummyEngineMatchApi {
        node_points: Vec<NodePoint>,
    }

    impl EngineMatchInlinePhaseApi for DummyEngineMatchApi {
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

    struct DummyEngineParseApi;

    impl EngineParseInlinePhaseApi for DummyEngineParseApi {
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

        fn parse_inline_tokens(&self, _tokens: &[InlineToken]) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn engine_match_should_find_two_space_hard_break() {
        let tokenizer = BreakTokenizer::default();
        let node_points = create_node_point_generator("foo  \nbaz")
            .pop()
            .expect("expected node points");
        let api = DummyEngineMatchApi { node_points };

        let mut hook = tokenizer.create_match_hook(&api);
        let delimiter = hook
            .find_delimiter(0, api.get_block_end_index())
            .expect("expected break delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 3);
        assert_eq!(delimiter.end_index, 5);
    }

    #[test]
    fn engine_parse_should_create_break_node() {
        let tokenizer = BreakTokenizer::default();
        let api = DummyEngineParseApi;
        let parse_hook = tokenizer.create_parse_hook(&api);

        let token = InlineToken::new(BREAK_TOKENIZER_NAME, BREAK_TYPE, (3, 5));
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0], Node::Break(_)));
    }
}
