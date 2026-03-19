use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_ast::HTML_TYPE;
#[cfg(test)]
use yozora_character::NodePoint;
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const HTML_INLINE_TOKENIZER_NAME: &str = "@yozora/tokenizer-html-inline";

#[derive(Debug, Clone)]
pub struct HtmlInlineTokenizer {
    meta: TokenizerMeta,
}

impl Default for HtmlInlineTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: HTML_INLINE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 10,
            },
        }
    }
}

impl Tokenizer for HtmlInlineTokenizer {
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

struct HtmlInlineMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for HtmlInlineMatchHook<'_> {
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
                let entry = r#match::find_delimiter_entry(
                    self.api.get_node_points(),
                    start_index,
                    end_index,
                )?;
                Some(entry.delimiter)
            },
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        r#match::process_single_delimiter(delimiter)
    }
}

struct HtmlInlineParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for HtmlInlineParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_html_inline_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for HtmlInlineTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(HtmlInlineMatchHook {
            api,
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(HtmlInlineParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::Node;
    use yozora_character::create_node_point_generator;

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

        fn calc_position(&self, _interval: NodeInterval) -> Option<yozora_ast::Position> {
            None
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

        fn parse_inline_tokens(&self, _tokens: &[InlineToken]) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn engine_match_should_find_html_delimiter() {
        let tokenizer = HtmlInlineTokenizer::default();
        let node_points = create_node_point_generator("<kbd>x</kbd>")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let mut hook = tokenizer.r#match(&api);
        let delimiter = hook
            .findDelimiter((0, api.get_block_end_index()))
            .expect("expected html delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 5);
    }

    #[test]
    fn engine_parse_should_build_html_node() {
        let tokenizer = HtmlInlineTokenizer::default();
        let node_points = create_node_point_generator("<kbd>x</kbd>")
            .pop()
            .expect("expected node points");
        let api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(HTML_INLINE_TOKENIZER_NAME, HTML_TYPE, (0, 12));
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        let Node::Html(html) = &nodes[0] else {
            panic!("expected html node");
        };
        assert_eq!(html.value, "<kbd>x</kbd>");
    }
}
