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
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for BreakMatchHook<'_> {
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
                r#match::find_break_delimiter(self.api, start_index, end_index)
            },
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
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
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(BreakMatchHook {
            api,
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn parse<'a>(
        &'a self,
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
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

        let mut match_hook = tokenizer.r#match(&match_api);
        let delimiter = match_hook
            .findDelimiter((0, match_api.get_block_end_index()))
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

        let mut hook = tokenizer.r#match(&api);
        let delimiter = hook
            .findDelimiter((0, api.get_block_end_index()))
            .expect("expected break delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 3);
        assert_eq!(delimiter.end_index, 5);
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
