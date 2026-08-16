use std::cell::RefCell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::types::{ImageDelimiter, IMAGE_TOKENIZER_NAME};
use crate::{parse, r#match};

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

pub struct ImageDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    block_end_index: usize,
}

impl ImageDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<ImageDelimiter> {
        r#match::find_image_delimiter_entry(
            self.api.get_node_points(),
            self.block_end_index,
            range_index.0,
            range_index.1,
        )
    }
}

pub struct ImageMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    block_end_index: usize,
    delimiters: Rc<RefCell<Vec<ImageDelimiter>>>,
}

impl<'a> ImageMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        let block_end_index = api.get_block_end_index();

        Self {
            api,
            block_end_index,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn find_delimiter(&self) -> ImageDelimiterGenerator<'a> {
        ImageDelimiterGenerator {
            api: self.api,
            block_end_index: self.block_end_index,
        }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &ImageDelimiter,
        closer_delimiter: &ImageDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        let status = check_balanced_brackets_status(
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

    pub fn process_delimiter_pair(
        &self,
        opener_delimiter: &ImageDelimiter,
        closer_delimiter: &ImageDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let children_tokens = self.api.resolve_internal_tokens(
            internal_tokens,
            opener_delimiter.end_index,
            closer_delimiter.start_index,
        );

        let token =
            r#match::create_image_token(opener_delimiter, closer_delimiter, children_tokens);

        ProcessDelimiterPairResult {
            tokens: vec![token],
            remain_opener_delimiter: None,
            remain_closer_delimiter: None,
        }
    }

    fn lookup_delimiter(&self, delimiter: &TokenDelimiter) -> Option<ImageDelimiter> {
        self.delimiters
            .borrow()
            .iter()
            .rev()
            .find(|candidate| {
                candidate.start_index == delimiter.start_index
                    && candidate.end_index == delimiter.end_index
                    && candidate.delimiter_type == delimiter.delimiter_type
            })
            .cloned()
    }
}

impl<'a> MatchInlineHook<'a> for ImageMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();

        let mut finder = ImageMatchHook::find_delimiter(self);
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(gen_find_delimiter(move |start_index, end_index| {
            let delimiter = finder.next((start_index, end_index))?;
            let core_delimiter = delimiter.to_core();
            delimiters.borrow_mut().push(delimiter);
            Some(core_delimiter)
        }))
    }

    fn is_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        let Some(opener_delimiter) = self.lookup_delimiter(opener_delimiter) else {
            return IsDelimiterPairResult::NotPaired {
                opener: false,
                closer: false,
            };
        };
        let Some(closer_delimiter) = self.lookup_delimiter(closer_delimiter) else {
            return IsDelimiterPairResult::NotPaired {
                opener: false,
                closer: false,
            };
        };

        ImageMatchHook::is_delimiter_pair(
            self,
            &opener_delimiter,
            &closer_delimiter,
            internal_tokens,
        )
    }

    fn process_delimiter_pair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let Some(opener_delimiter) = self.lookup_delimiter(opener_delimiter) else {
            return ProcessDelimiterPairResult {
                tokens: Vec::new(),
                remain_opener_delimiter: None,
                remain_closer_delimiter: None,
            };
        };
        let Some(closer_delimiter) = self.lookup_delimiter(closer_delimiter) else {
            return ProcessDelimiterPairResult {
                tokens: Vec::new(),
                remain_opener_delimiter: None,
                remain_closer_delimiter: None,
            };
        };

        ImageMatchHook::process_delimiter_pair(
            self,
            &opener_delimiter,
            &closer_delimiter,
            internal_tokens,
        )
    }
}

pub struct ImageParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> ImageParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
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
        Box::new(ImageParseHook::new(api))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::{create_node_point_generator, NodePoint};

    struct DummyMatchApi {
        node_points: Vec<NodePoint>,
    }

    impl MatchInlinePhaseApi for DummyMatchApi {
        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn get_node_points(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn get_block_start_index(&self) -> usize {
            0
        }

        fn get_block_end_index(&self) -> usize {
            self.node_points.len()
        }

        fn resolve_fallback_tokens(
            &self,
            tokens: &[InlineToken],
            _token_start_index: usize,
            _token_end_index: usize,
        ) -> Vec<InlineToken> {
            tokens.to_vec()
        }

        fn resolve_internal_tokens(
            &self,
            _higher_priority_tokens: &[InlineToken],
            start_index: usize,
            end_index: usize,
        ) -> Vec<InlineToken> {
            vec![InlineToken::new(
                "text",
                yozora_ast::TEXT_TYPE,
                (start_index, end_index),
            )]
        }
    }

    #[test]
    fn typed_hook_preserves_destination_and_title() {
        let node_points = create_node_point_generator("![foo](/uri \"title\")")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };
        let hook = ImageMatchHook::new(&api);
        let mut finder = hook.find_delimiter();
        let opener = finder
            .next((0, api.get_block_end_index()))
            .expect("expected opener");
        let closer = finder
            .next((opener.end_index, api.get_block_end_index()))
            .expect("expected closer");

        assert_eq!(
            closer.destination_content,
            Some(NodeInterval {
                start_index: 7,
                end_index: 11,
            })
        );
        assert_eq!(
            closer.title_content,
            Some(NodeInterval {
                start_index: 12,
                end_index: 19,
            })
        );
    }
}
