use yozora_ast::{FootnoteReference, Node, FOOTNOTE_REFERENCE_TYPE};
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

use crate::r#match::FootnoteReferenceToken;
use crate::{parse, r#match};

pub const FOOTNOTE_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-reference";

#[derive(Debug, Clone)]
pub struct FootnoteReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteReferenceTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FOOTNOTE_REFERENCE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 7,
            },
        }
    }
}

impl Tokenizer for FootnoteReferenceTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for FootnoteReferenceTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_footnote_reference_tokens(input, None)?;
        Some(parse::parse_footnote_reference_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_footnote_reference_tokens(input, Some(api))?;
        Some(parse::parse_footnote_reference_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        match_api: &dyn MatchInlinePhaseApi,
        parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_footnote_reference_tokens(input, Some(match_api))?;
        Some(parse::parse_footnote_reference_tokens(
            input,
            &tokens,
            Some(parse_api),
        ))
    }
}

#[derive(Debug, Clone)]
struct FootnoteRefTokenData {
    identifier: String,
    label: String,
}

impl EngineTokenizer for FootnoteReferenceTokenizer {
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
    data: FootnoteRefTokenData,
}

struct FootnoteReferenceMatchHook {
    delimiters: Vec<DelimiterEntry>,
    cursor: usize,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl FootnoteReferenceMatchHook {
    fn new(api: &dyn EngineMatchInlinePhaseApi) -> Self {
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();
        let node_points = api.get_node_points();

        let source = build_source(node_points, block_start_index, block_end_index);
        let char_starts = build_char_starts(&source);

        let adapter = LegacyMatchApiAdapter { api };
        let matched =
            r#match::match_footnote_reference_tokens(&source, Some(&adapter)).unwrap_or_default();

        let mut delimiters = Vec::new();
        for token in matched {
            let FootnoteReferenceToken::Reference {
                interval,
                identifier,
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
                data: FootnoteRefTokenData { identifier, label },
            });
        }

        Self {
            delimiters,
            cursor: 0,
            last_end_index: None,
            last_delimiter: None,
        }
    }

    fn lookup_data(&self, delimiter: &TokenDelimiter) -> Option<&FootnoteRefTokenData> {
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

impl MatchInlineHook for FootnoteReferenceMatchHook {
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
            FOOTNOTE_REFERENCE_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )
        .with_data(data.clone())]
    }
}

struct FootnoteReferenceParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for FootnoteReferenceParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<FootnoteRefTokenData>() else {
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

            nodes.push(Node::FootnoteReference(FootnoteReference {
                position,
                identifier: data.identifier.clone(),
                label: data.label.clone(),
            }));
        }

        nodes
    }
}

impl EngineInlineTokenizer for FootnoteReferenceTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(FootnoteReferenceMatchHook::new(api))
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(FootnoteReferenceParseHook { api })
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

    struct DummyInlineApi {
        footnotes: std::collections::HashSet<String>,
    }

    impl DummyInlineApi {
        fn from(footnotes: &[&str]) -> Self {
            Self {
                footnotes: footnotes.iter().map(|v| v.to_string()).collect(),
            }
        }
    }

    impl MatchInlinePhaseApi for DummyInlineApi {
        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, identifier: &str) -> bool {
            self.footnotes.contains(identifier)
        }
    }

    #[test]
    fn phase_api_should_filter_out_unknown_footnote() {
        let tokenizer = FootnoteReferenceTokenizer::default();
        let api = DummyInlineApi::from(&[]);

        let nodes = tokenizer.tokenize_inline_with_api("[^missing]", None, &api);
        assert!(nodes.is_none(), "unexpected nodes: {nodes:?}");
    }

    #[test]
    fn phase_api_should_accept_known_footnote() {
        let tokenizer = FootnoteReferenceTokenizer::default();
        let api = DummyInlineApi::from(&["note"]);

        let nodes = tokenizer
            .tokenize_inline_with_api("[^note]", None, &api)
            .expect("should parse known footnote");
        assert!(matches!(&nodes[0], Node::FootnoteReference(_)));
    }
}
