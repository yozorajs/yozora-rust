use std::cell::RefCell;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const IMAGE_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-image-reference";

#[derive(Debug, Clone)]
pub struct ImageReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for ImageReferenceTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: IMAGE_REFERENCE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: TokenizerPriority::LINKS,
            },
        }
    }
}

impl Tokenizer for ImageReferenceTokenizer {
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

struct ImageReferenceMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    delimiters: RefCell<Vec<r#match::DelimiterEntry>>,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl ImageReferenceMatchHook<'_> {
    fn register_delimiter(&self, entry: r#match::DelimiterEntry) {
        self.delimiters.borrow_mut().push(entry);
    }

    fn lookup_brackets(
        &self,
        delimiter: &TokenDelimiter,
    ) -> Vec<r#match::ImageReferenceDelimiterBracket> {
        self.delimiters
            .borrow()
            .iter()
            .rev()
            .find(|entry| {
                entry.delimiter.start_index == delimiter.start_index
                    && entry.delimiter.end_index == delimiter.end_index
                    && entry.delimiter.delimiter_type == delimiter.delimiter_type
            })
            .map(|entry| entry.brackets.clone())
            .unwrap_or_default()
    }
}

impl MatchInlineHook for ImageReferenceMatchHook<'_> {
    fn reset(&mut self) {
        self.last_end_index = None;
        self.last_delimiter = None;
        self.delimiters.borrow_mut().clear();
    }

    fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        let mut last_end_index = self.last_end_index;
        let mut last_delimiter = self.last_delimiter.clone();
        let delimiter = genFindDelimiter(
            range_index,
            &mut last_end_index,
            &mut last_delimiter,
            |start_index, end_index| {
                let entry = r#match::find_image_reference_delimiter_entry(
                    self.api.getNodePoints(),
                    start_index,
                    end_index,
                )?;
                let delimiter = entry.delimiter.clone();
                self.register_delimiter(entry);
                Some(delimiter)
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
        let status = r#match::check_balanced_brackets_status(
            opener_delimiter.end_index,
            closer_delimiter.start_index,
            internal_tokens,
            self.api.getNodePoints(),
        );

        match status {
            -1 => IsDelimiterPairResult::NotPaired {
                opener: false,
                closer: true,
            },
            0 => IsDelimiterPairResult::Paired,
            1 => IsDelimiterPairResult::NotPaired {
                opener: true,
                closer: false,
            },
            _ => IsDelimiterPairResult::NotPaired {
                opener: false,
                closer: false,
            },
        }
    }

    fn processDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let brackets = self.lookup_brackets(closer_delimiter);
        let tokens = r#match::process_delimiter_pair(
            self.api,
            opener_delimiter,
            closer_delimiter,
            &brackets,
            internal_tokens,
            self.api.getNodePoints(),
        );

        ProcessDelimiterPairResult {
            tokens,
            remainOpenerDelimiter: None,
            remainCloserDelimiter: None,
        }
    }
}

struct ImageReferenceParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for ImageReferenceParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_image_reference_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for ImageReferenceTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(ImageReferenceMatchHook {
            api,
            delimiters: RefCell::new(Vec::new()),
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(ImageReferenceParseHook { api })
    }
}
