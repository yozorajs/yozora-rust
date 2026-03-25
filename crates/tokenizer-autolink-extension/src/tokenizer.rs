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

use crate::{parse, r#match};

pub const AUTOLINK_EXTENSION_TOKENIZER_NAME: &str = "@yozora/tokenizer-autolink-extension";

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

struct AutolinkExtensionMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    delimiters: Rc<RefCell<Vec<r#match::DelimiterEntry>>>,
}

impl AutolinkExtensionMatchHook<'_> {
    fn lookup_content_type(
        &self,
        delimiter: &TokenDelimiter,
    ) -> Option<parse::AutolinkExtensionContentType> {
        self.delimiters
            .borrow()
            .iter()
            .rev()
            .find(|entry| {
                entry.delimiter.start_index == delimiter.start_index
                    && entry.delimiter.end_index == delimiter.end_index
                    && entry.delimiter.delimiter_type == delimiter.delimiter_type
            })
            .map(|entry| entry.content_type)
    }
}

impl<'a> MatchInlineHook<'a> for AutolinkExtensionMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();
        let api = self.api;
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(genFindDelimiter(move |start_index, end_index| {
            let entry = r#match::find_delimiter_entry(
                api.getNodePoints(),
                api.getBlockStartIndex(),
                start_index,
                end_index,
            )?;
            let delimiter = entry.delimiter.clone();
            delimiters.borrow_mut().push(entry);
            Some(delimiter)
        }))
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(content_type) = self.lookup_content_type(delimiter) else {
            return Vec::new();
        };

        r#match::process_single_delimiter(self.api, delimiter, content_type)
    }
}

struct AutolinkExtensionParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for AutolinkExtensionParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_autolink_extension_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for AutolinkExtensionTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(AutolinkExtensionMatchHook {
            api,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(AutolinkExtensionParseHook { api })
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
            token_start_index: usize,
            token_end_index: usize,
        ) -> Vec<InlineToken> {
            vec![InlineToken::new(
                "text",
                TEXT_TYPE,
                (token_start_index, token_end_index),
            )]
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

        fn parseInlineTokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node> {
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
        let mut find_delimiter = hook.findDelimiter();
        let delimiter = find_delimiter
            .next((0, api.getBlockEndIndex()))
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 6);
        assert_eq!(delimiter.end_index, 21);
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
            .with_data(parse::AutolinkExtensionTokenData {
                content_type: parse::AutolinkExtensionContentType::UriWww,
                children_tokens: vec![InlineToken::new("text", TEXT_TYPE, (0, 15))],
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
