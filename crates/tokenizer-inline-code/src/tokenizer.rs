use yozora_ast::{Node, INLINE_CODE_TYPE};
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const INLINE_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-inline-code";

#[derive(Debug, Clone)]
pub struct InlineCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for InlineCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: INLINE_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: TokenizerPriority::ATOMIC,
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

struct InlineCodeMatchHook {
    delimiter_finder: r#match::InlineCodeDelimiterFinder,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for InlineCodeMatchHook {
    fn reset(&mut self) {
        self.delimiter_finder.reset();
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
            |start_index, _end_index| self.delimiter_finder.find_next_delimiter(start_index),
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        vec![InlineToken::new(
            "",
            INLINE_CODE_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )
        .with_data(parse::InlineCodeTokenData {
            thickness: delimiter.thickness,
        })]
    }
}

struct InlineCodeParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
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
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(InlineCodeMatchHook {
            delimiter_finder: r#match::InlineCodeDelimiterFinder::new(api),
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn parse<'a>(
        &'a self,
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(InlineCodeParseHook { api })
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
            Vec::new()
        }
    }

    struct DummyParseApi {
        node_points: Vec<NodePoint>,
    }

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
            &self.node_points
        }

        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn parseInlineTokens(&self, _tokens: Option<&[InlineToken]>) -> Vec<Node> {
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

        let mut hook = tokenizer.r#match(&api);
        let delimiter = hook
            .findDelimiter((0, api.getBlockEndIndex()))
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
            .with_data(parse::InlineCodeTokenData { thickness: 1 });
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        let Node::InlineCode(inline_code) = &nodes[0] else {
            panic!("expected inline code node");
        };
        assert_eq!(inline_code.value, "foo");
    }
}
