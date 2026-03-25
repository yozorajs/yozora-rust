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
        Self::new(TokenizerOptions::default())
    }
}

impl HtmlInlineTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| HTML_INLINE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
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
}

impl<'a> MatchInlineHook<'a> for HtmlInlineMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let api = self.api;

        Box::new(genFindDelimiter(|start_index, end_index| {
            let entry = r#match::find_delimiter_entry(api.getNodePoints(), start_index, end_index)?;
            Some(entry.delimiter)
        }))
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
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(HtmlInlineMatchHook { api })
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
    fn engine_match_should_find_html_delimiter() {
        let tokenizer = HtmlInlineTokenizer::default();
        let node_points = create_node_point_generator("<kbd>x</kbd>")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.findDelimiter();
        let delimiter = find_delimiter
            .next((0, api.getBlockEndIndex()))
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
