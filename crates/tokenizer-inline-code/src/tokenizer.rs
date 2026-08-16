use yozora_ast::{Node, INLINE_CODE_TYPE};
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{InlineCodeDelimiter, InlineCodeTokenData, INLINE_CODE_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct InlineCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for InlineCodeTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl InlineCodeTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| INLINE_CODE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for InlineCodeTokenizer {
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

pub struct InlineCodeDelimiterGenerator {
    finder: r#match::InlineCodeDelimiterFinder,
}

impl InlineCodeDelimiterGenerator {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<InlineCodeDelimiter> {
        self.finder.find_next_delimiter(range_index.0)
    }
}

pub struct InlineCodeMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> InlineCodeMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self { api }
    }

    pub fn find_delimiter(&self) -> InlineCodeDelimiterGenerator {
        InlineCodeDelimiterGenerator {
            finder: r#match::InlineCodeDelimiterFinder::new(self.api),
        }
    }

    pub fn process_single_delimiter(&self, delimiter: &InlineCodeDelimiter) -> Vec<InlineToken> {
        vec![InlineToken::new(
            "",
            INLINE_CODE_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )
        .with_data(InlineCodeTokenData {
            thickness: delimiter.thickness,
        })]
    }
}

impl<'a> MatchInlineHook<'a> for InlineCodeMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let mut finder = InlineCodeMatchHook::find_delimiter(self);

        Box::new(gen_find_delimiter(move |start_index, _end_index| {
            finder.next((start_index, _end_index))
        }))
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        InlineCodeMatchHook::process_single_delimiter(self, delimiter)
    }
}

pub struct InlineCodeParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> InlineCodeParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for InlineCodeParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_inline_code_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for InlineCodeTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(InlineCodeMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(InlineCodeParseHook::new(api))
    }
}

#[cfg(test)]
mod tests {
    use yozora_character::{create_node_point_generator, NodePoint};

    use super::*;

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

        fn calc_position(&self, _interval: NodeInterval) -> yozora_ast::Position {
            panic!("calc_position should not be called in this test")
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

        fn parse_inline_tokens(&self, _tokens: Option<&[InlineToken]>) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn engine_match_should_emit_full_backtick_delimiter() {
        let tokenizer = InlineCodeTokenizer::default();
        let node_points = create_node_point_generator("`foo`")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, api.get_block_end_index()))
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 5);
        assert_eq!(delimiter.thickness, 1);
    }

    #[test]
    fn engine_parse_should_trim_boundary_space_like() {
        let tokenizer = InlineCodeTokenizer::default();
        let node_points = create_node_point_generator("` foo `")
            .pop()
            .expect("expected node points");

        let parse_api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&parse_api);

        let token = InlineToken::new(INLINE_CODE_TOKENIZER_NAME, INLINE_CODE_TYPE, (0, 7))
            .with_data(InlineCodeTokenData { thickness: 1 });
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        let Node::InlineCode(inline_code) = &nodes[0] else {
            panic!("expected inline code node");
        };
        assert_eq!(inline_code.value, "foo");
    }
}
