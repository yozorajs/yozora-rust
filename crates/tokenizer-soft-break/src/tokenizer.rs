use yozora_ast::{Node, Text, TEXT_TYPE};
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

pub const SOFT_BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-soft-break";

#[derive(Debug, Clone)]
pub struct SoftBreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for SoftBreakTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: SOFT_BREAK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 0,
            },
        }
    }
}

impl Tokenizer for SoftBreakTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for SoftBreakTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let value = r#match::match_soft_break_text(input)?;
        Some(parse::parse_soft_break_text(value))
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
struct SoftBreakTokenData {
    value: String,
}

impl EngineTokenizer for SoftBreakTokenizer {
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

struct SoftBreakMatchHook {
    delimiter: Option<TokenDelimiter>,
    data: Option<SoftBreakTokenData>,
}

impl SoftBreakMatchHook {
    fn new(api: &dyn EngineMatchInlinePhaseApi) -> Self {
        let start_index = api.get_block_start_index();
        let end_index = api.get_block_end_index();
        let source = build_source(api.get_node_points(), start_index, end_index);

        let Some(value) = r#match::match_soft_break_text(&source) else {
            return Self {
                delimiter: None,
                data: None,
            };
        };

        Self {
            delimiter: Some(TokenDelimiter {
                delimiter_type: DelimiterType::Full,
                start_index,
                end_index,
                thickness: end_index.saturating_sub(start_index),
                original_thickness: end_index.saturating_sub(start_index),
            }),
            data: Some(SoftBreakTokenData { value }),
        }
    }
}

impl MatchInlineHook for SoftBreakMatchHook {
    fn find_delimiter(&mut self, start_index: usize, end_index: usize) -> Option<TokenDelimiter> {
        let delimiter = self.delimiter.clone()?;
        if delimiter.start_index < start_index || delimiter.end_index > end_index {
            return None;
        }

        self.delimiter = None;
        Some(delimiter)
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(data) = self.data.as_ref() else {
            return Vec::new();
        };

        vec![
            InlineToken::new("", TEXT_TYPE, (delimiter.start_index, delimiter.end_index))
                .with_data(data.clone()),
        ]
    }
}

struct SoftBreakParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for SoftBreakParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<SoftBreakTokenData>() else {
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

            nodes.push(Node::Text(Text {
                position,
                value: data.value.clone(),
            }));
        }

        nodes
    }
}

impl EngineInlineTokenizer for SoftBreakTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(SoftBreakMatchHook::new(api))
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(SoftBreakParseHook { api })
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
