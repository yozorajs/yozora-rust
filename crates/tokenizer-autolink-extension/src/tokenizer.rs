use std::cell::RefCell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_ast::{Text, LINK_TYPE, TEXT_TYPE};
#[cfg(test)]
use yozora_character::{calc_string_from_node_points, NodePoint};
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{AutolinkExtensionDelimiter, AUTOLINK_EXTENSION_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct AutolinkExtensionTokenizer {
    meta: TokenizerMeta,
}

impl Default for AutolinkExtensionTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl AutolinkExtensionTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| AUTOLINK_EXTENSION_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::LINKS),
            },
        }
    }
}

impl Tokenizer for AutolinkExtensionTokenizer {
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

pub struct AutolinkExtensionDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl AutolinkExtensionDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<AutolinkExtensionDelimiter> {
        r#match::find_delimiter_entry(
            self.api.get_node_points(),
            self.api.get_block_start_index(),
            range_index.0,
            range_index.1,
        )
    }
}

pub struct AutolinkExtensionMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    delimiters: Rc<RefCell<Vec<AutolinkExtensionDelimiter>>>,
}

impl<'a> AutolinkExtensionMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self {
            api,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn find_delimiter(&self) -> AutolinkExtensionDelimiterGenerator<'a> {
        AutolinkExtensionDelimiterGenerator { api: self.api }
    }

    pub fn process_single_delimiter(
        &self,
        delimiter: &AutolinkExtensionDelimiter,
    ) -> Vec<InlineToken> {
        r#match::process_single_delimiter(self.api, delimiter)
    }

    fn lookup_delimiter(&self, delimiter: &TokenDelimiter) -> Option<AutolinkExtensionDelimiter> {
        self.delimiters
            .borrow()
            .iter()
            .rev()
            .find(|candidate| candidate.to_core() == *delimiter)
            .cloned()
    }
}

impl<'a> MatchInlineHook<'a> for AutolinkExtensionMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();
        let mut finder = AutolinkExtensionMatchHook::find_delimiter(self);
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(gen_find_delimiter(move |start_index, end_index| {
            let delimiter = finder.next((start_index, end_index))?;
            let core_delimiter = delimiter.to_core();
            delimiters.borrow_mut().push(delimiter);
            Some(core_delimiter)
        }))
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(delimiter) = self.lookup_delimiter(delimiter) else {
            return Vec::new();
        };

        AutolinkExtensionMatchHook::process_single_delimiter(self, &delimiter)
    }
}

pub struct AutolinkExtensionParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> AutolinkExtensionParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for AutolinkExtensionParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_autolink_extension_tokens(tokens, self.api).resolve(self.api)
    }
}

impl InlineTokenizer for AutolinkExtensionTokenizer {
    fn parse_deferred<'a>(
        &'a self,
        tokens: &'a [InlineToken],
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Option<ParseInlineHookResult<'a>> {
        Some(parse::parse_autolink_extension_tokens(tokens, api))
    }

    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(AutolinkExtensionMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(AutolinkExtensionParseHook::new(api))
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
            token_start_index: usize,
            token_end_index: usize,
        ) -> Vec<InlineToken> {
            vec![InlineToken::new(
                "text",
                TEXT_TYPE,
                (token_start_index, token_end_index),
            )]
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

        fn parse_inline_tokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node> {
            let Some(tokens) = tokens else {
                return Vec::new();
            };
            tokens
                .iter()
                .map(|token| {
                    Node::Text(Text {
                        position: None,
                        value: calc_string_from_node_points(
                            &self.node_points,
                            token.start_index,
                            token.end_index,
                            false,
                        ),
                    })
                })
                .collect()
        }
    }

    #[test]
    fn engine_match_should_find_autolink_extension_delimiter() {
        let tokenizer = AutolinkExtensionTokenizer::default();
        let node_points = create_node_point_generator("hello www.example.com")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, api.get_block_end_index()))
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 6);
        assert_eq!(delimiter.end_index, 21);
    }

    #[test]
    fn typed_hook_preserves_content_type() {
        let node_points = create_node_point_generator("hello www.example.com")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };
        let hook = AutolinkExtensionMatchHook::new(&api);
        let mut finder = hook.find_delimiter();
        let delimiter = finder
            .next((0, api.get_block_end_index()))
            .expect("expected autolink extension delimiter");

        assert_eq!(
            delimiter.content_type,
            crate::types::AutolinkExtensionContentType::UriWww
        );
    }

    #[test]
    fn engine_parse_should_build_link_node() {
        let tokenizer = AutolinkExtensionTokenizer::default();
        let node_points = create_node_point_generator("www.example.com")
            .pop()
            .expect("expected node points");
        let api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(AUTOLINK_EXTENSION_TOKENIZER_NAME, LINK_TYPE, (0, 15))
            .with_children(vec![InlineToken::new("text", TEXT_TYPE, (0, 15))])
            .with_data(crate::types::AutolinkExtensionTokenData {
                content_type: crate::types::AutolinkExtensionContentType::UriWww,
            });

        let nodes = parse_hook.parse(&[token]);
        assert_eq!(nodes.len(), 1);

        let Node::Link(link) = &nodes[0] else {
            panic!("expected link node");
        };

        assert_eq!(link.url, "http://www.example.com");
        assert_eq!(link.children.len(), 1);
    }
}
