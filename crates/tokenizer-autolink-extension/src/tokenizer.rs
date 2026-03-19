use yozora_ast::{Link, Node, Text, LINK_TYPE};
use yozora_character::{NodePoint, VirtualCodePoint};
use yozora_core_tokenizer::engine::{
    DelimiterType, EngineInlineTokenizer, EngineTokenizer, InlineToken, MatchInlineHook,
    MatchInlinePhaseApi as EngineMatchInlinePhaseApi, ParseInlineHook,
    ParseInlinePhaseApi as EngineParseInlinePhaseApi, TokenDelimiter, TokenizerType,
};
use yozora_core_tokenizer::phase::NodeInterval;
use yozora_core_tokenizer::{
    InlineTokenizer, MatchInlinePhaseApi, ParseInlinePhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::r#match::AutolinkExtensionToken;
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
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for AutolinkExtensionTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_autolink_extension_tokens(input)?;
        Some(parse::parse_autolink_extension_tokens(input, &tokens, None))
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
        let tokens = r#match::match_autolink_extension_tokens(input)?;
        Some(parse::parse_autolink_extension_tokens(
            input,
            &tokens,
            Some(parse_api),
        ))
    }
}

#[derive(Debug, Clone)]
struct AutolinkExtTokenData {
    display: String,
    href: String,
}

impl EngineTokenizer for AutolinkExtensionTokenizer {
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

#[derive(Debug, Clone)]
struct DelimiterEntry {
    delimiter: TokenDelimiter,
    data: AutolinkExtTokenData,
}

struct AutolinkExtensionMatchHook {
    delimiters: Vec<DelimiterEntry>,
    cursor: usize,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl AutolinkExtensionMatchHook {
    fn new(api: &dyn EngineMatchInlinePhaseApi) -> Self {
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();
        let node_points = api.get_node_points();

        let source = build_source(node_points, block_start_index, block_end_index);
        let char_starts = build_char_starts(&source);
        let matched = r#match::match_autolink_extension_tokens(&source).unwrap_or_default();

        let mut delimiters = Vec::new();
        for token in matched {
            let AutolinkExtensionToken::Link {
                interval,
                display,
                href,
            } = token
            else {
                continue;
            };

            let local_start = byte_to_char_index(&char_starts, source.len(), interval.start_index);
            let local_end = byte_to_char_index(&char_starts, source.len(), interval.end_index);
            if local_start >= local_end {
                continue;
            }

            delimiters.push(DelimiterEntry {
                delimiter: TokenDelimiter {
                    delimiter_type: DelimiterType::Full,
                    start_index: block_start_index + local_start,
                    end_index: block_start_index + local_end,
                    thickness: local_end - local_start,
                    original_thickness: local_end - local_start,
                },
                data: AutolinkExtTokenData { display, href },
            });
        }

        Self {
            delimiters,
            cursor: 0,
            last_end_index: None,
            last_delimiter: None,
        }
    }

    fn lookup_data(&self, delimiter: &TokenDelimiter) -> Option<&AutolinkExtTokenData> {
        self.delimiters
            .iter()
            .find(|x| {
                x.delimiter.start_index == delimiter.start_index
                    && x.delimiter.end_index == delimiter.end_index
            })
            .map(|x| &x.data)
    }

    fn find_delimiter_impl(
        &mut self,
        start_index: usize,
        end_index: usize,
    ) -> Option<TokenDelimiter> {
        while self.cursor < self.delimiters.len() {
            let delimiter = &self.delimiters[self.cursor].delimiter;
            if delimiter.start_index < start_index {
                self.cursor += 1;
                continue;
            }
            if delimiter.start_index >= end_index {
                return None;
            }

            self.cursor += 1;
            return Some(delimiter.clone());
        }

        None
    }
}

impl MatchInlineHook for AutolinkExtensionMatchHook {
    fn reset(&mut self) {
        self.cursor = 0;
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

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(data) = self.lookup_data(delimiter) else {
            return Vec::new();
        };

        vec![
            InlineToken::new("", LINK_TYPE, (delimiter.start_index, delimiter.end_index))
                .with_data(data.clone()),
        ]
    }
}

struct AutolinkExtensionParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for AutolinkExtensionParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<AutolinkExtTokenData>() else {
                continue;
            };

            let position = if self.api.should_reserve_position() {
                self.api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                })
            } else {
                None
            };

            nodes.push(Node::Link(Link {
                position,
                url: self.api.format_url(&data.href),
                title: None,
                children: vec![Node::Text(Text {
                    position: None,
                    value: data.display.clone(),
                })],
            }));
        }

        nodes
    }
}

impl EngineInlineTokenizer for AutolinkExtensionTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(AutolinkExtensionMatchHook::new(api))
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(AutolinkExtensionParseHook { api })
    }
}

fn build_source(node_points: &[NodePoint], start_index: usize, end_index: usize) -> String {
    let mut source = String::new();
    for point in node_points
        .iter()
        .skip(start_index)
        .take(end_index.saturating_sub(start_index))
    {
        let code_point = point.code_point;
        let ch = if code_point == VirtualCodePoint::Space as i32 {
            Some(' ')
        } else if code_point == VirtualCodePoint::LineEnd as i32 {
            Some('\n')
        } else {
            char::from_u32(code_point as u32)
        };

        if let Some(ch) = ch {
            source.push(ch);
        }
    }
    source
}

fn build_char_starts(source: &str) -> Vec<usize> {
    source.char_indices().map(|(i, _)| i).collect()
}

fn byte_to_char_index(char_starts: &[usize], source_len: usize, byte_index: usize) -> usize {
    if byte_index >= source_len {
        return char_starts.len();
    }

    match char_starts.binary_search(&byte_index) {
        Ok(i) | Err(i) => i,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::create_node_point_generator;

    struct DummyMatchApi {
        node_points: Vec<NodePoint>,
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
            Vec::new()
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

        fn parse_inline_tokens(&self, _tokens: &[InlineToken]) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn engine_match_should_find_autolink_extension_delimiter() {
        let tokenizer = AutolinkExtensionTokenizer::default();
        let node_points = create_node_point_generator("www.example.com")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let mut hook = tokenizer.create_match_hook(&api);
        let delimiter = hook
            .find_delimiter(0, api.get_block_end_index())
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 0);
    }

    #[test]
    fn engine_parse_should_build_link_node() {
        let tokenizer = AutolinkExtensionTokenizer::default();
        let api = DummyParseApi;
        let parse_hook = tokenizer.create_parse_hook(&api);

        let token = InlineToken::new(AUTOLINK_EXTENSION_TOKENIZER_NAME, LINK_TYPE, (0, 15))
            .with_data(AutolinkExtTokenData {
                display: "www.example.com".to_string(),
                href: "http://www.example.com".to_string(),
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
