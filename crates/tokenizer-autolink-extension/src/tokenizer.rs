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
        Self {
            meta: TokenizerMeta {
                name: AUTOLINK_EXTENSION_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 4,
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
    delimiters: Vec<r#match::DelimiterEntry>,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl AutolinkExtensionMatchHook<'_> {
    fn register_delimiter(&mut self, entry: r#match::DelimiterEntry) {
        self.delimiters.push(entry);
    }

    fn lookup_content_type(
        &self,
        delimiter: &TokenDelimiter,
    ) -> Option<parse::AutolinkExtensionContentType> {
        self.delimiters
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

impl MatchInlineHook for AutolinkExtensionMatchHook<'_> {
    fn reset(&mut self) {
        self.last_end_index = None;
        self.last_delimiter = None;
        self.delimiters.clear();
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
                    self.api.get_block_start_index(),
                    start_index,
                    end_index,
                )?;
                let delimiter = entry.delimiter.clone();
                self.register_delimiter(entry);
                Some(delimiter)
            },
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
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
    fn r#match<'a>(&'a self, api: &'a dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(AutolinkExtensionMatchHook {
            api,
            delimiters: Vec::new(),
            last_end_index: None,
            last_delimiter: None,
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

        fn parse_inline_tokens(&self, tokens: &[InlineToken]) -> Vec<Node> {
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

        let mut hook = tokenizer.r#match(&api);
        let delimiter = hook
            .findDelimiter((0, api.get_block_end_index()))
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
