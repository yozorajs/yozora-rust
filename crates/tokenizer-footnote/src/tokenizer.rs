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
        Self {
            meta: TokenizerMeta {
                name: FOOTNOTE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: TokenizerPriority::LINKS,
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
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for FootnoteMatchHook<'_> {
    fn reset(&mut self) {
        self.last_end_index = None;
        self.last_delimiter = None;
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
                    self.api.getNodePoints(),
                    start_index,
                    end_index,
                )?;
                Some(entry.delimiter)
            },
        );

        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
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
    fn r#match<'a>(&'a self, api: &'a dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(FootnoteMatchHook {
            api,
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(FootnoteParseHook { api })
    }
}
