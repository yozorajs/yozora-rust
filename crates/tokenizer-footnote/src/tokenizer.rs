use yozora_ast::{Footnote, Node, Text, FOOTNOTE_TYPE};
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

pub const FOOTNOTE_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote";

#[derive(Debug, Clone)]
pub struct FootnoteTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FOOTNOTE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                // containing-inline 语义需要在 emphasis / inline-code 之前处理。
                priority: 13,
            },
        }
    }
}

impl Tokenizer for FootnoteTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for FootnoteTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_footnote_tokens(input)?;
        Some(parse::parse_footnote_tokens(input, &tokens))
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
enum FootnoteMatchData {
    Footnote { content: String },
    Literal { value: String },
}

impl EngineTokenizer for FootnoteTokenizer {
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
    data: FootnoteMatchData,
}

struct FootnoteMatchHook {
    delimiters: Vec<DelimiterEntry>,
    cursor: usize,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl FootnoteMatchHook {
    fn new(api: &dyn EngineMatchInlinePhaseApi) -> Self {
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();
        let node_points = api.get_node_points();

        let source = build_source(node_points, block_start_index, block_end_index);
        let char_starts = build_char_starts(&source);
        let delimiters = collect_delimiters(&source, &char_starts, block_start_index);

        Self {
            delimiters,
            cursor: 0,
            last_end_index: None,
            last_delimiter: None,
        }
    }

    fn lookup_data(&self, delimiter: &TokenDelimiter) -> Option<&FootnoteMatchData> {
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

impl MatchInlineHook for FootnoteMatchHook {
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
            FOOTNOTE_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )
        .with_data(data.clone())]
    }
}

struct FootnoteParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for FootnoteParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<FootnoteMatchData>() else {
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

            match data {
                FootnoteMatchData::Footnote { content } => {
                    nodes.push(Node::Footnote(Footnote {
                        position,
                        children: vec![Node::Text(Text {
                            position: None,
                            value: content.clone(),
                        })],
                    }));
                }
                FootnoteMatchData::Literal { value } => {
                    nodes.push(Node::Text(Text {
                        position,
                        value: value.clone(),
                    }));
                }
            }
        }

        nodes
    }
}

impl EngineInlineTokenizer for FootnoteTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(FootnoteMatchHook::new(api))
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(FootnoteParseHook { api })
    }
}

fn collect_delimiters(
    source: &str,
    char_starts: &[usize],
    block_start_index: usize,
) -> Vec<DelimiterEntry> {
    let mut delimiters = Vec::new();
    let bytes = source.as_bytes();

    let mut cursor = 0usize;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] != b'^' || bytes[cursor + 1] != b'[' {
            cursor += 1;
            continue;
        }

        if is_escaped(source, cursor) {
            let start = cursor.saturating_sub(1);
            let end = cursor + 2;
            push_delimiter(
                &mut delimiters,
                char_starts,
                source.len(),
                block_start_index,
                start,
                end,
                FootnoteMatchData::Literal {
                    value: "^[".to_string(),
                },
            );
            cursor += 2;
            continue;
        }

        let content_start = cursor + 2;
        let Some(end_bracket) = find_matching_bracket(source, content_start) else {
            break;
        };

        push_delimiter(
            &mut delimiters,
            char_starts,
            source.len(),
            block_start_index,
            cursor,
            end_bracket + 1,
            FootnoteMatchData::Footnote {
                content: source[content_start..end_bracket].to_string(),
            },
        );

        cursor = end_bracket + 1;
    }

    delimiters
}

fn push_delimiter(
    delimiters: &mut Vec<DelimiterEntry>,
    char_starts: &[usize],
    source_len: usize,
    block_start_index: usize,
    start_byte: usize,
    end_byte: usize,
    data: FootnoteMatchData,
) {
    let local_start = byte_to_char_index(char_starts, source_len, start_byte);
    let local_end = byte_to_char_index(char_starts, source_len, end_byte);
    if local_start >= local_end {
        return;
    }

    delimiters.push(DelimiterEntry {
        delimiter: TokenDelimiter {
            delimiter_type: DelimiterType::Full,
            start_index: block_start_index + local_start,
            end_index: block_start_index + local_end,
            thickness: local_end - local_start,
            original_thickness: local_end - local_start,
        },
        data,
    });
}

fn is_escaped(source: &str, byte_index: usize) -> bool {
    if byte_index == 0 {
        return false;
    }

    let bytes = source.as_bytes();
    let mut idx = byte_index;
    let mut slash_count = 0usize;

    while idx > 0 {
        idx -= 1;
        if bytes[idx] == b'\\' {
            slash_count += 1;
            continue;
        }
        break;
    }

    slash_count % 2 == 1
}

fn find_matching_bracket(source: &str, content_start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 1usize;
    let mut index = content_start;
    let mut code_ticks = 0usize;

    while index < bytes.len() {
        if code_ticks > 0 {
            if bytes[index] == b'`' {
                let run = count_repeat(bytes, index, b'`');
                if run >= code_ticks {
                    code_ticks = 0;
                }
                index += run;
                continue;
            }

            index += 1;
            continue;
        }

        match bytes[index] {
            b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            b'`' => {
                code_ticks = count_repeat(bytes, index, b'`');
                index += code_ticks;
            }
            b'[' => {
                depth += 1;
                index += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
                index += 1;
            }
            _ => index += 1,
        }
    }

    None
}

fn count_repeat(bytes: &[u8], mut index: usize, target: u8) -> usize {
    let mut count = 0usize;
    while index < bytes.len() && bytes[index] == target {
        count += 1;
        index += 1;
    }
    count
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
