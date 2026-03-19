use std::cell::RefCell;

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
        Self {
            meta: TokenizerMeta {
                name: IMAGE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 5,
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
    delimiters: RefCell<Vec<r#match::DelimiterEntry>>,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl<'a> ImageMatchHook<'a> {
    fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();
        let source =
            r#match::build_source(api.get_node_points(), block_start_index, block_end_index);
        let char_starts = r#match::build_char_starts(&source);

        Self {
            api,
            source,
            char_starts,
            block_start_index,
            block_end_index,
            delimiters: RefCell::new(Vec::new()),
            last_end_index: None,
            last_delimiter: None,
        }
    }

    fn register_delimiter(&self, entry: r#match::DelimiterEntry) {
        self.delimiters.borrow_mut().push(entry);
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

impl MatchInlineHook for ImageMatchHook<'_> {
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
                let entry = r#match::find_image_delimiter_entry(
                    &self.source,
                    &self.char_starts,
                    self.api.get_node_points(),
                    self.block_start_index,
                    self.block_end_index,
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
            self.api.get_node_points(),
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

        let children_tokens = self.api.resolve_internal_tokens(
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
    fn r#match<'b>(&'b self, api: &'b dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook + 'b> {
        Box::new(ImageMatchHook::new(api))
    }

    fn parse<'b>(&'b self, api: &'b dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'b> {
        Box::new(ImageParseHook { api })
    }
}
