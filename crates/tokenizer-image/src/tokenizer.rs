use std::cell::RefCell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const IMAGE_TOKENIZER_NAME: &str = "@yozora/tokenizer-image";

#[derive(Debug, Clone)]
pub struct ImageTokenizer {
    meta: TokenizerMeta,
}

impl Default for ImageTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl ImageTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| IMAGE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::LINKS),
            },
        }
    }
}

impl Tokenizer for ImageTokenizer {
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

struct ImageMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    source: String,
    char_starts: Vec<usize>,
    block_start_index: usize,
    block_end_index: usize,
    delimiters: Rc<RefCell<Vec<r#match::DelimiterEntry>>>,
}

impl<'a> ImageMatchHook<'a> {
    fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        let block_start_index = api.getBlockStartIndex();
        let block_end_index = api.getBlockEndIndex();
        let source = r#match::build_source(api.getNodePoints(), block_start_index, block_end_index);
        let char_starts = r#match::build_char_starts(&source);

        Self {
            api,
            source,
            char_starts,
            block_start_index,
            block_end_index,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn lookup_data(&self, delimiter: &TokenDelimiter) -> Option<r#match::ImageDelimiterData> {
        self.delimiters
            .borrow()
            .iter()
            .rev()
            .find(|entry| {
                entry.delimiter.start_index == delimiter.start_index
                    && entry.delimiter.end_index == delimiter.end_index
                    && entry.delimiter.delimiter_type == delimiter.delimiter_type
            })
            .and_then(|entry| entry.data.clone())
    }
}

impl<'a> MatchInlineHook<'a> for ImageMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();

        let source = self.source.clone();
        let char_starts = self.char_starts.clone();
        let api = self.api;
        let block_start_index = self.block_start_index;
        let block_end_index = self.block_end_index;
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(genFindDelimiter(move |start_index, end_index| {
            let entry = r#match::find_image_delimiter_entry(
                &source,
                &char_starts,
                api.getNodePoints(),
                block_start_index,
                block_end_index,
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
        let Some(data) = self.lookup_data(closer_delimiter) else {
            return ProcessDelimiterPairResult {
                tokens: Vec::new(),
                remainOpenerDelimiter: None,
                remainCloserDelimiter: None,
            };
        };

        let children_tokens = self.api.resolveInternalTokens(
            internal_tokens,
            opener_delimiter.end_index,
            closer_delimiter.start_index,
        );

        let token = r#match::create_image_token(
            opener_delimiter,
            closer_delimiter,
            data.destination_content,
            data.title_content,
            children_tokens,
        );

        ProcessDelimiterPairResult {
            tokens: vec![token],
            remainOpenerDelimiter: None,
            remainCloserDelimiter: None,
        }
    }
}

struct ImageParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for ImageParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_image_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for ImageTokenizer {
    fn r#match<'b>(
        &'b self,
        api: &'b dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'b> + 'b> {
        Box::new(ImageMatchHook::new(api))
    }

    fn parse<'b>(&'b self, api: &'b dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'b> {
        Box::new(ImageParseHook { api })
    }
}
