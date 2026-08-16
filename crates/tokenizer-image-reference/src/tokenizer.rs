use std::cell::RefCell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::types::{ImageReferenceDelimiter, IMAGE_REFERENCE_TOKENIZER_NAME};
use crate::{parse, r#match};

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

pub struct ImageReferenceDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl ImageReferenceDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<ImageReferenceDelimiter> {
        r#match::find_image_reference_delimiter_entry(
            self.api.get_node_points(),
            range_index.0,
            range_index.1,
        )
    }
}

pub struct ImageReferenceMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    delimiters: Rc<RefCell<Vec<ImageReferenceDelimiter>>>,
}

impl<'a> ImageReferenceMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self {
            api,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn find_delimiter(&self) -> ImageReferenceDelimiterGenerator<'a> {
        ImageReferenceDelimiterGenerator { api: self.api }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &ImageReferenceDelimiter,
        closer_delimiter: &ImageReferenceDelimiter,
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
        opener_delimiter: &ImageReferenceDelimiter,
        closer_delimiter: &ImageReferenceDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let tokens = r#match::process_delimiter_pair(
            self.api,
            opener_delimiter,
            closer_delimiter,
            internal_tokens,
        );

        ProcessDelimiterPairResult {
            tokens,
            remain_opener_delimiter: None,
            remain_closer_delimiter: None,
        }
    }

    fn lookup_delimiter(&self, delimiter: &TokenDelimiter) -> Option<ImageReferenceDelimiter> {
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

impl<'a> MatchInlineHook<'a> for ImageReferenceMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();

        let mut finder = ImageReferenceMatchHook::find_delimiter(self);
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

        ImageReferenceMatchHook::is_delimiter_pair(
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

        ImageReferenceMatchHook::process_delimiter_pair(
            self,
            &opener_delimiter,
            &closer_delimiter,
            internal_tokens,
        )
    }
}

pub struct ImageReferenceParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> ImageReferenceParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
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
        Box::new(ImageReferenceMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(ImageReferenceParseHook::new(api))
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
        fn has_definition(&self, identifier: &str) -> bool {
            identifier == "bar"
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
    fn typed_hook_preserves_reference_brackets() {
        let node_points = create_node_point_generator("![foo][bar]")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };
        let hook = ImageReferenceMatchHook::new(&api);
        let mut finder = hook.find_delimiter();
        let opener = finder
            .next((0, api.get_block_end_index()))
            .expect("expected opener");
        let closer = finder
            .next((opener.end_index, api.get_block_end_index()))
            .expect("expected closer");

        assert_eq!(closer.brackets.len(), 1);
        assert_eq!(closer.brackets[0].label.as_deref(), Some("bar"));
        assert_eq!(closer.brackets[0].identifier.as_deref(), Some("bar"));
    }
}
