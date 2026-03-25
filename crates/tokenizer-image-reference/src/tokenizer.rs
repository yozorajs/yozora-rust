use std::cell::RefCell;
use std::rc::Rc;

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
        Self::new(TokenizerOptions::default())
    }
}

impl ImageReferenceTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| IMAGE_REFERENCE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::LINKS),
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
    delimiters: Rc<RefCell<Vec<r#match::DelimiterEntry>>>,
}

impl ImageReferenceMatchHook<'_> {
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

impl<'a> MatchInlineHook<'a> for ImageReferenceMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();

        let api = self.api;
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(genFindDelimiter(move |start_index, end_index| {
            let entry = r#match::find_image_reference_delimiter_entry(
                api.getNodePoints(),
                start_index,
                end_index,
            )?;
            let delimiter = entry.delimiter.clone();
            delimiters.borrow_mut().push(entry);
            Some(delimiter)
        }))
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
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(ImageReferenceMatchHook {
            api,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(ImageReferenceParseHook { api })
    }
}
