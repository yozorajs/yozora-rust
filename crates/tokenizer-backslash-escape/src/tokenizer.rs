use yozora_ast::{Node, Text, TEXT_TYPE};
use yozora_character::{NodePoint, VirtualCodePoint};
use yozora_core_tokenizer::NodeInterval;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const BACKSLASH_ESCAPE_TOKENIZER_NAME: &str = "@yozora/tokenizer-backslash-escape";

#[derive(Debug, Clone)]
pub struct BackslashEscapeTokenizer {
    meta: TokenizerMeta,
}

impl Default for BackslashEscapeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: BACKSLASH_ESCAPE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: TokenizerPriority::SOFT_INLINE,
            },
        }
    }
}



#[derive(Debug, Clone)]
struct EscapeTokenData {
    value: String,
}

impl Tokenizer for BackslashEscapeTokenizer {
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

struct BackslashEscapeMatchHook {
    delimiter: Option<TokenDelimiter>,
    data: Option<EscapeTokenData>,
}

impl BackslashEscapeMatchHook {
    fn new(api: &dyn MatchInlinePhaseApi) -> Self {
        let start_index = api.getBlockStartIndex();
        let end_index = api.getBlockEndIndex();
        let source = build_source(api.getNodePoints(), start_index, end_index);

        let Some(value) = r#match::match_backslash_escaped_text(&source) else {
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
            data: Some(EscapeTokenData { value }),
        }
    }
}

impl MatchInlineHook for BackslashEscapeMatchHook {
    fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        let (start_index, end_index) = range_index;
        let delimiter = self.delimiter.clone()?;
        if delimiter.start_index < start_index || delimiter.end_index > end_index {
            return None;
        }

        self.delimiter = None;
        Some(delimiter)
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(data) = self.data.as_ref() else {
            return Vec::new();
        };

        vec![
            InlineToken::new("", TEXT_TYPE, (delimiter.start_index, delimiter.end_index))
                .with_data(data.clone()),
        ]
    }
}

struct BackslashEscapeParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for BackslashEscapeParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<EscapeTokenData>() else {
                continue;
            };

            let position = if self.api.shouldReservePosition() {
                Some(self.api.calcPosition(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
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

impl InlineTokenizer for BackslashEscapeTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(BackslashEscapeMatchHook::new(api))
    }

    fn parse<'a>(
        &'a self,
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(BackslashEscapeParseHook { api })
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
