use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const FOOTNOTE_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote";

#[derive(Debug, Clone)]
pub struct FootnoteTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl FootnoteTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| FOOTNOTE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::LINKS),
            },
        }
    }
}

impl Tokenizer for FootnoteTokenizer {
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

struct FootnoteMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> MatchInlineHook<'a> for FootnoteMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let api = self.api;

        Box::new(genFindDelimiter(|start_index, end_index| {
            let entry = r#match::find_delimiter_entry(api.getNodePoints(), start_index, end_index)?;
            Some(entry.delimiter)
        }))
    }

    fn isDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        r#match::is_delimiter_pair(
            self.api,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }

    fn processDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        r#match::process_delimiter_pair(
            self.api,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        )
    }
}

struct FootnoteParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for FootnoteParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_footnote_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for FootnoteTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(FootnoteMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(FootnoteParseHook { api })
    }
}
