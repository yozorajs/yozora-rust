use yozora_ast::{LinkReference, Node, Text, LINK_REFERENCE_TYPE};
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

use crate::{parse, r#match};

pub const LINK_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-link-reference";

#[derive(Debug, Clone)]
pub struct LinkReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for LinkReferenceTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: LINK_REFERENCE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 6,
            },
        }
    }
}

impl Tokenizer for LinkReferenceTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for LinkReferenceTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_link_reference_tokens(input, None)?;
        Some(parse::parse_link_reference_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_link_reference_tokens(input, Some(api))?;
        Some(parse::parse_link_reference_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        match_api: &dyn MatchInlinePhaseApi,
        parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_link_reference_tokens(input, Some(match_api))?;
        Some(parse::parse_link_reference_tokens(
            input,
            &tokens,
            Some(parse_api),
        ))
    }
}

#[derive(Debug, Clone)]
struct LinkReferenceTokenData {
    identifier: String,
    label: String,
    reference_type: yozora_ast::ReferenceType,
    child_text: String,
}

impl EngineTokenizer for LinkReferenceTokenizer {
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

struct LegacyMatchApiAdapter<'a> {
    api: &'a dyn EngineMatchInlinePhaseApi,
}

impl MatchInlinePhaseApi for LegacyMatchApiAdapter<'_> {
    fn has_definition(&self, identifier: &str) -> bool {
        self.api.has_definition(identifier)
    }

    fn has_footnote_definition(&self, identifier: &str) -> bool {
        self.api.has_footnote_definition(identifier)
    }
}

#[derive(Debug, Clone)]
struct DelimiterEntry {
    delimiter: TokenDelimiter,
    data: LinkReferenceTokenData,
}

struct LinkReferenceMatchHook {
    delimiters: Vec<DelimiterEntry>,
    cursor: usize,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl LinkReferenceMatchHook {
    fn new(api: &dyn EngineMatchInlinePhaseApi) -> Self {
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();
        let node_points = api.get_node_points();

        let source = build_source(node_points, block_start_index, block_end_index);
        let char_starts = build_char_starts(&source);

        let adapter = LegacyMatchApiAdapter { api };
        let matched =
            r#match::match_link_reference_tokens(&source, Some(&adapter)).unwrap_or_default();

        let mut delimiters = Vec::new();
        for token in matched {
            let r#match::LinkReferenceToken::Link {
                interval,
                identifier,
                label,
                reference_type,
                child_text,
                ..
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
                data: LinkReferenceTokenData {
                    identifier,
                    label,
                    reference_type,
                    child_text,
                },
            });
        }

        Self {
            delimiters,
            cursor: 0,
            last_end_index: None,
            last_delimiter: None,
        }
    }

    fn lookup_data(&self, delimiter: &TokenDelimiter) -> Option<&LinkReferenceTokenData> {
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

impl MatchInlineHook for LinkReferenceMatchHook {
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

        vec![InlineToken::new(
            "",
            LINK_REFERENCE_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )
        .with_data(data.clone())]
    }
}

struct LinkReferenceParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for LinkReferenceParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<LinkReferenceTokenData>() else {
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

            nodes.push(Node::LinkReference(LinkReference {
                position,
                identifier: data.identifier.clone(),
                label: data.label.clone(),
                reference_type: data.reference_type,
                children: vec![Node::Text(Text {
                    position: None,
                    value: data.child_text.clone(),
                })],
            }));
        }

        nodes
    }
}

impl EngineInlineTokenizer for LinkReferenceTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(LinkReferenceMatchHook::new(api))
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(LinkReferenceParseHook { api })
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
    use yozora_ast::{ReferenceType, Text};

    struct DummyInlineApi {
        definitions: std::collections::HashSet<String>,
    }

    impl DummyInlineApi {
        fn from(definitions: &[&str]) -> Self {
            Self {
                definitions: definitions.iter().map(|v| v.to_string()).collect(),
            }
        }
    }

    impl MatchInlinePhaseApi for DummyInlineApi {
        fn has_definition(&self, identifier: &str) -> bool {
            self.definitions.contains(identifier)
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }
    }

    #[test]
    fn should_not_capture_inline_link_with_nested_brackets() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer.tokenize_inline("[link [foo [bar]]](/uri)", None);
        assert!(nodes.is_none(), "got: {:?}", nodes);
    }

    #[test]
    fn should_not_capture_inline_link_with_image_label() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer.tokenize_inline("[![moon](moon.jpg)](/uri)", None);
        assert!(nodes.is_none());
    }

    #[test]
    fn should_capture_full_reference_with_inline_content() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer.tokenize_inline("[link *foo **bar** `#`*][ref]", None);
        assert!(nodes.is_some(), "nodes should be parsed");

        let nodes = nodes.unwrap();
        assert_eq!(nodes.len(), 1, "got: {nodes:?}");
        let Node::LinkReference(link) = &nodes[0] else {
            panic!("expected linkReference, got: {nodes:?}");
        };
        assert_eq!(link.reference_type, ReferenceType::Full);
        assert_eq!(link.label, "ref");
    }

    #[test]
    fn should_prefer_middle_full_reference_in_three_labels() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[foo][bar][baz]", None)
            .expect("nodes");

        let Node::LinkReference(link) = &nodes[0] else {
            panic!("got: {nodes:?}");
        };
        assert_eq!(link.reference_type, ReferenceType::Full);
        assert_eq!(link.label, "bar");
    }

    #[test]
    fn should_parse_inner_shortcut_for_double_wrapped_label() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[[*foo* bar]]", None)
            .expect("nodes");

        assert!(matches!(&nodes[0], Node::Text(Text { value, .. }) if value == "["));
        assert!(
            matches!(&nodes[1], Node::LinkReference(_)),
            "got: {nodes:?}"
        );
        assert!(matches!(&nodes[2], Node::Text(Text { value, .. }) if value == "]"));
    }

    #[test]
    fn phase_api_should_filter_out_unknown_definition() {
        let tokenizer = LinkReferenceTokenizer::default();
        let api = DummyInlineApi::from(&[]);

        let nodes = tokenizer.tokenize_inline_with_api("[foo][bar]", None, &api);
        assert!(nodes.is_none(), "unexpected nodes: {nodes:?}");
    }

    #[test]
    fn phase_api_should_accept_known_definition() {
        let tokenizer = LinkReferenceTokenizer::default();
        let api = DummyInlineApi::from(&["bar"]);

        let nodes = tokenizer
            .tokenize_inline_with_api("[foo][bar]", None, &api)
            .expect("should parse known definition");
        assert!(matches!(&nodes[0], Node::LinkReference(_)));
    }
}
