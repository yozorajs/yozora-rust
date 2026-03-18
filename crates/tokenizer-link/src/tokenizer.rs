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

use crate::r#match::LinkToken;
use crate::{parse, r#match};

pub const LINK_TOKENIZER_NAME: &str = "@yozora/tokenizer-link";

#[derive(Debug, Clone)]
pub struct LinkTokenizer {
    meta: TokenizerMeta,
}

impl Default for LinkTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: LINK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 9,
            },
        }
    }
}

impl Tokenizer for LinkTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for LinkTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_link_tokens(input)?;
        Some(parse::parse_link_tokens(input, &tokens))
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        position: Option<yozora_ast::Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, position)
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        position: Option<yozora_ast::Position>,
        _match_api: &dyn MatchInlinePhaseApi,
        _parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, position)
    }
}

#[derive(Debug, Clone)]
struct LinkTokenData {
    url: String,
    title: Option<String>,
    label: String,
}

impl EngineTokenizer for LinkTokenizer {
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
    data: LinkTokenData,
}

struct LinkMatchHook {
    delimiters: Vec<DelimiterEntry>,
    cursor: usize,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl LinkMatchHook {
    fn new(api: &dyn EngineMatchInlinePhaseApi) -> Self {
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();
        let node_points = api.get_node_points();

        let source = build_source(node_points, block_start_index, block_end_index);
        let char_starts = build_char_starts(&source);
        let matched = r#match::match_link_tokens(&source).unwrap_or_default();

        let mut delimiters = Vec::new();
        for token in matched {
            let LinkToken::Link {
                interval,
                url,
                title,
                label,
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
                data: LinkTokenData { url, title, label },
            });
        }

        Self {
            delimiters,
            cursor: 0,
            last_end_index: None,
            last_delimiter: None,
        }
    }

    fn lookup_data(&self, delimiter: &TokenDelimiter) -> Option<&LinkTokenData> {
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

impl MatchInlineHook for LinkMatchHook {
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

struct LinkParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for LinkParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<LinkTokenData>() else {
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
                url: data.url.clone(),
                title: data.title.clone(),
                children: vec![Node::Text(Text {
                    position: None,
                    value: data.label.clone(),
                })],
            }));
        }

        nodes
    }
}

impl EngineInlineTokenizer for LinkTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(LinkMatchHook::new(api))
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(LinkParseHook { api })
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
    use yozora_ast::Node;

    #[test]
    fn parses_empty_pointy_destination() {
        let tokenizer = LinkTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[link](<>)", None)
            .expect("should parse");

        assert_eq!(nodes.len(), 1);
        let Node::Link(link) = &nodes[0] else {
            panic!("expected link node");
        };
        assert_eq!(link.url, "");
        assert!(link.title.is_none());
    }

    #[test]
    fn parses_parenthesized_title() {
        let tokenizer = LinkTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[link](/url (title))", None)
            .expect("should parse");

        let Node::Link(link) = &nodes[0] else {
            panic!("expected link node");
        };
        assert_eq!(link.url, "/url");
        assert_eq!(link.title.as_deref(), Some("title"));
    }

    #[test]
    fn parses_balanced_brackets_in_text() {
        let tokenizer = LinkTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[link [foo [bar]]](/uri)", None)
            .expect("should parse");

        let Node::Link(link) = &nodes[0] else {
            panic!("expected link node");
        };
        assert_eq!(link.url, "/uri");
        assert_eq!(link.children.len(), 1);
        let Node::Text(text) = &link.children[0] else {
            panic!("expected text child");
        };
        assert_eq!(text.value, "link [foo [bar]]");
    }

    #[test]
    fn forbids_nested_links() {
        let tokenizer = LinkTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[foo [bar](/uri)](/uri)", None)
            .expect("should parse partially");

        assert_eq!(nodes.len(), 3);
        let Node::Text(prefix) = &nodes[0] else {
            panic!("expected text prefix");
        };
        assert_eq!(prefix.value, "[foo ");

        let Node::Link(inner) = &nodes[1] else {
            panic!("expected inner link");
        };
        assert_eq!(inner.url, "/uri");

        let Node::Text(suffix) = &nodes[2] else {
            panic!("expected text suffix");
        };
        assert_eq!(suffix.value, "](/uri)");
    }
}
