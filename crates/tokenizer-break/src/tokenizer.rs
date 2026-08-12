use yozora_ast::{Node, BREAK_TYPE};
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-break";

#[derive(Debug, Clone)]
pub struct BreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for BreakTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl BreakTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| BREAK_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::SOFT_INLINE),
            },
        }
    }
}

impl Tokenizer for BreakTokenizer {
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

struct BreakMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> MatchInlineHook<'a> for BreakMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let api = self.api;

        Box::new(gen_find_delimiter(|start_index, end_index| {
            r#match::find_break_delimiter(api, start_index, end_index)
        }))
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
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for BreakParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_break_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for BreakTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(BreakMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(BreakParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::{create_node_point_generator, NodePoint};

    #[test]
    fn phase_api_should_preserve_break_result() {
        let tokenizer = BreakTokenizer::default();
        let node_points = create_node_point_generator("line  \nnext")
            .pop()
            .expect("expected node points");
        let match_api = DummyEngineMatchApi { node_points };

        let match_hook = tokenizer.r#match(&match_api);
        let mut find_delimiter = match_hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, match_api.get_block_end_index()))
            .expect("expected break delimiter");

        let parse_api = DummyEngineParseApi;
        let parse_hook = tokenizer.parse(&parse_api);
        let tokens = vec![InlineToken::new(
            BREAK_TOKENIZER_NAME,
            BREAK_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )];
        let nodes = parse_hook.parse(&tokens);

        assert!(matches!(nodes.first(), Some(Node::Break(_))));
    }

    struct DummyEngineMatchApi {
        node_points: Vec<NodePoint>,
    }

    impl MatchInlinePhaseApi for DummyEngineMatchApi {
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

    impl ParseInlinePhaseApi for DummyEngineParseApi {
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

        fn parse_inline_tokens(&self, _tokens: Option<&[InlineToken]>) -> Vec<Node> {
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

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, api.get_block_end_index()))
            .expect("expected break delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 3);
        assert_eq!(delimiter.end_index, 6);
    }

    #[test]
    fn engine_parse_should_create_break_node() {
        let tokenizer = BreakTokenizer::default();
        let api = DummyEngineParseApi;
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(BREAK_TOKENIZER_NAME, BREAK_TYPE, (3, 5));
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0], Node::Break(_)));
    }
}
