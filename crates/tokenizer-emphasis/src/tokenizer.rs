use yozora_ast::{Emphasis, Node, Strong, EMPHASIS_TYPE, STRONG_TYPE};
use yozora_character::{
    is_punctuation_character, is_unicode_whitespace_character, AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::engine::{
    DelimiterType, EngineInlineTokenizer, EngineTokenizer, InlineToken, IsDelimiterPairResult,
    MatchInlineHook, MatchInlinePhaseApi as EngineMatchInlinePhaseApi, ParseInlineHook,
    ParseInlinePhaseApi as EngineParseInlinePhaseApi, ProcessDelimiterPairResult, TokenDelimiter,
    TokenizerType,
};
use yozora_core_tokenizer::phase::NodeInterval;
use yozora_core_tokenizer::{
    InlineTokenizer, MatchInlinePhaseApi, ParseInlinePhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const EMPHASIS_TOKENIZER_NAME: &str = "@yozora/tokenizer-emphasis";

#[derive(Debug, Clone)]
pub struct EmphasisTokenizer {
    meta: TokenizerMeta,
}

impl Default for EmphasisTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: EMPHASIS_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                // Run after link/image/reference grouping.
                priority: 4,
            },
        }
    }
}

impl Tokenizer for EmphasisTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for EmphasisTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let matched = r#match::match_emphasis(input)?;
        Some(parse::parse_emphasis(input, &matched, None))
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, None)
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        _match_api: &dyn MatchInlinePhaseApi,
        parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let matched = r#match::match_emphasis(input)?;
        Some(parse::parse_emphasis(input, &matched, Some(parse_api)))
    }
}

#[derive(Debug, Clone)]
struct EmphasisTokenData {
    thickness: usize,
    children: Vec<InlineToken>,
}

impl EngineTokenizer for EmphasisTokenizer {
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

struct EmphasisMatchHook<'a> {
    api: &'a dyn EngineMatchInlinePhaseApi,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl EmphasisMatchHook<'_> {
    fn is_opener_delimiter(
        node_points: &[NodePoint],
        delimiter_start_index: usize,
        delimiter_end_index: usize,
        block_end_index: usize,
        start_index: usize,
        end_index: usize,
    ) -> bool {
        if delimiter_end_index == block_end_index {
            return false;
        }
        if delimiter_end_index == end_index {
            return true;
        }

        let Some(next) = node_points.get(delimiter_end_index) else {
            return false;
        };
        if is_unicode_whitespace_character(next.code_point) {
            return false;
        }

        if !is_punctuation_character(next.code_point) {
            return true;
        }

        if delimiter_start_index <= start_index {
            return true;
        }

        let prev = node_points[delimiter_start_index - 1].code_point;
        is_unicode_whitespace_character(prev) || is_punctuation_character(prev)
    }

    fn is_closer_delimiter(
        node_points: &[NodePoint],
        delimiter_start_index: usize,
        delimiter_end_index: usize,
        block_start_index: usize,
        start_index: usize,
        end_index: usize,
    ) -> bool {
        if delimiter_start_index == block_start_index {
            return false;
        }
        if delimiter_start_index == start_index {
            return true;
        }

        let prev = node_points[delimiter_start_index - 1].code_point;
        if is_unicode_whitespace_character(prev) {
            return false;
        }

        if !is_punctuation_character(prev) {
            return true;
        }

        if delimiter_end_index >= end_index {
            return true;
        }

        let next = node_points[delimiter_end_index].code_point;
        is_unicode_whitespace_character(next) || is_punctuation_character(next)
    }

    fn find_delimiter_impl(&self, start_index: usize, end_index: usize) -> Option<TokenDelimiter> {
        let node_points = self.api.get_node_points();
        let block_start_index = self.api.get_block_start_index();
        let block_end_index = self.api.get_block_end_index();

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

            if c != AsciiCodePoint::ASTERISK as i32 && c != AsciiCodePoint::UNDERSCORE as i32 {
                i += 1;
                continue;
            }

            let delimiter_start_index = i;
            i += 1;
            while i < end_index && node_points[i].code_point == c {
                i += 1;
            }
            let delimiter_end_index = i;

            let is_left_flanking = Self::is_opener_delimiter(
                node_points,
                delimiter_start_index,
                delimiter_end_index,
                block_end_index,
                start_index,
                end_index,
            );
            let is_right_flanking = Self::is_closer_delimiter(
                node_points,
                delimiter_start_index,
                delimiter_end_index,
                block_start_index,
                start_index,
                end_index,
            );

            let mut is_opener = is_left_flanking;
            let mut is_closer = is_right_flanking;

            if c == AsciiCodePoint::UNDERSCORE as i32 && is_left_flanking && is_right_flanking {
                if delimiter_start_index > start_index
                    && !is_punctuation_character(node_points[delimiter_start_index - 1].code_point)
                {
                    is_opener = false;
                }

                let next_is_punctuation = node_points
                    .get(delimiter_end_index)
                    .is_some_and(|p| is_punctuation_character(p.code_point));
                if !next_is_punctuation {
                    is_closer = false;
                }
            }

            if !is_opener && !is_closer {
                continue;
            }

            let thickness = delimiter_end_index - delimiter_start_index;
            return Some(TokenDelimiter {
                delimiter_type: match (is_opener, is_closer) {
                    (true, true) => DelimiterType::Both,
                    (true, false) => DelimiterType::Opener,
                    (false, true) => DelimiterType::Closer,
                    (false, false) => unreachable!(),
                },
                start_index: delimiter_start_index,
                end_index: delimiter_end_index,
                thickness,
                original_thickness: thickness,
            });
        }

        None
    }
}

impl MatchInlineHook for EmphasisMatchHook<'_> {
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

    fn is_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        _internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        let node_points = self.api.get_node_points();

        let Some(opener) = node_points.get(opener_delimiter.start_index) else {
            return IsDelimiterPairResult::NotPaired {
                opener: true,
                closer: true,
            };
        };
        let Some(closer) = node_points.get(closer_delimiter.start_index) else {
            return IsDelimiterPairResult::NotPaired {
                opener: true,
                closer: true,
            };
        };

        let is_same_marker = opener.code_point == closer.code_point;
        let violates_mod_three = (matches!(opener_delimiter.delimiter_type, DelimiterType::Both)
            || matches!(closer_delimiter.delimiter_type, DelimiterType::Both))
            && (opener_delimiter.original_thickness + closer_delimiter.original_thickness) % 3 == 0
            && opener_delimiter.original_thickness % 3 != 0;

        if !is_same_marker || violates_mod_three {
            return IsDelimiterPairResult::NotPaired {
                opener: true,
                closer: true,
            };
        }

        IsDelimiterPairResult::Paired
    }

    fn process_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let thickness = if opener_delimiter.thickness > 1 && closer_delimiter.thickness > 1 {
            2
        } else {
            1
        };

        let resolved_children = self.api.resolve_internal_tokens(
            internal_tokens,
            opener_delimiter.end_index,
            closer_delimiter.start_index,
        );

        let node_type = if thickness == 2 {
            STRONG_TYPE
        } else {
            EMPHASIS_TYPE
        };

        let token = InlineToken::new(
            "",
            node_type,
            (
                opener_delimiter.end_index - thickness,
                closer_delimiter.start_index + thickness,
            ),
        )
        .with_data(EmphasisTokenData {
            thickness,
            children: resolved_children,
        });

        let remain_opener_delimiter = if opener_delimiter.thickness > thickness {
            Some(TokenDelimiter {
                delimiter_type: opener_delimiter.delimiter_type,
                start_index: opener_delimiter.start_index,
                end_index: opener_delimiter.end_index - thickness,
                thickness: opener_delimiter.thickness - thickness,
                original_thickness: opener_delimiter.original_thickness,
            })
        } else {
            None
        };

        let remain_closer_delimiter = if closer_delimiter.thickness > thickness {
            Some(TokenDelimiter {
                delimiter_type: closer_delimiter.delimiter_type,
                start_index: closer_delimiter.start_index + thickness,
                end_index: closer_delimiter.end_index,
                thickness: closer_delimiter.thickness - thickness,
                original_thickness: closer_delimiter.original_thickness,
            })
        } else {
            None
        };

        ProcessDelimiterPairResult {
            tokens: vec![token],
            remain_opener_delimiter,
            remain_closer_delimiter,
        }
    }
}

struct EmphasisParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for EmphasisParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<EmphasisTokenData>() else {
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

            let is_strong = token.node_type == STRONG_TYPE || data.thickness == 2;
            if is_strong {
                nodes.push(Node::Strong(Strong { position, children }));
            } else {
                nodes.push(Node::Emphasis(Emphasis { position, children }));
            }
        }

        nodes
    }
}

impl EngineInlineTokenizer for EmphasisTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(EmphasisMatchHook {
            api,
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(EmphasisParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::TEXT_TYPE;
    use yozora_character::create_node_point_generator;

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
    fn engine_match_should_find_underscore_delimiter() {
        let tokenizer = EmphasisTokenizer::default();
        let node_points = create_node_point_generator("__foo__")
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

        let hook = tokenizer.create_match_hook(&api);
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
        assert_eq!(data.children.len(), resolved_tokens.len());
        assert_eq!(data.children[0].tokenizer, resolved_tokens[0].tokenizer);
        assert_eq!(data.children[0].node_type, resolved_tokens[0].node_type);
        assert_eq!(data.children[0].start_index, resolved_tokens[0].start_index);
        assert_eq!(data.children[0].end_index, resolved_tokens[0].end_index);
        assert!(result.remain_opener_delimiter.is_none());
        assert!(result.remain_closer_delimiter.is_none());
    }

    #[test]
    fn engine_parse_should_parse_children_tokens() {
        let tokenizer = EmphasisTokenizer::default();
        let api = DummyParseApi;
        let parse_hook = tokenizer.create_parse_hook(&api);

        let token = InlineToken::new(EMPHASIS_TOKENIZER_NAME, EMPHASIS_TYPE, (1, 4)).with_data(
            EmphasisTokenData {
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
