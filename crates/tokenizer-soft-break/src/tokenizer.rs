use yozora_ast::{Node, TEXT_TYPE};
use yozora_core_tokenizer::*;

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
                priority: TokenizerPriority::SOFT_INLINE,
            },
        }
    }
}

impl Tokenizer for SoftBreakTokenizer {
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

struct SoftBreakMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    last_delimiter: Option<TokenDelimiter>,
    last_data: Option<parse::SoftBreakTokenData>,
}

impl MatchInlineHook for SoftBreakMatchHook<'_> {
    fn reset(&mut self) {
        self.last_delimiter = None;
        self.last_data = None;
    }

    fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        let (start_index, end_index) = range_index;
        if let Some(delimiter) = &self.last_delimiter {
            if delimiter.start_index >= start_index && delimiter.end_index <= end_index {
                return Some(delimiter.clone());
            }
        }

        let Some(value) =
            r#match::match_soft_break_value(self.api.getNodePoints(), start_index, end_index)
        else {
            self.last_delimiter = None;
            self.last_data = None;
            return None;
        };

        let delimiter = TokenDelimiter {
            delimiter_type: DelimiterType::Full,
            start_index,
            end_index,
            thickness: end_index.saturating_sub(start_index),
            original_thickness: end_index.saturating_sub(start_index),
        };

        self.last_delimiter = Some(delimiter.clone());
        self.last_data = Some(parse::SoftBreakTokenData { value });
        Some(delimiter)
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(data) = self.last_data.as_ref() else {
            return Vec::new();
        };

        vec![
            InlineToken::new("", TEXT_TYPE, (delimiter.start_index, delimiter.end_index))
                .with_data(data.clone()),
        ]
    }
}

struct SoftBreakParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for SoftBreakParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_soft_break_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for SoftBreakTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(SoftBreakMatchHook {
            api,
            last_delimiter: None,
            last_data: None,
        })
    }

    fn parse<'a>(
        &'a self,
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(SoftBreakParseHook { api })
    }
}
