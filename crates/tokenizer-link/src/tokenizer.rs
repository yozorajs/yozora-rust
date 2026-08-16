use std::cell::RefCell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use crate::types::LinkTokenData;
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{LinkDelimiter, LINK_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct LinkTokenizer {
    meta: TokenizerMeta,
}

impl Default for LinkTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl LinkTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| LINK_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::LINKS),
            },
        }
    }
}

impl Tokenizer for LinkTokenizer {
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

pub struct LinkDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    block_start_index: usize,
    block_end_index: usize,
}

impl LinkDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<LinkDelimiter> {
        r#match::find_link_delimiter_entry(
            self.api.get_node_points(),
            self.block_start_index,
            self.block_end_index,
            range_index.0,
            range_index.1,
        )
    }
}

pub struct LinkMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    block_start_index: usize,
    block_end_index: usize,
    delimiters: Rc<RefCell<Vec<LinkDelimiter>>>,
}

impl<'a> LinkMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();
        Self {
            api,
            block_start_index,
            block_end_index,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn find_delimiter(&self) -> LinkDelimiterGenerator<'a> {
        LinkDelimiterGenerator {
            api: self.api,
            block_start_index: self.block_start_index,
            block_end_index: self.block_end_index,
        }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &LinkDelimiter,
        closer_delimiter: &LinkDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        if contains_link_token(
            internal_tokens,
            opener_delimiter.end_index,
            closer_delimiter.start_index,
        ) {
            return IsDelimiterPairResult::NotPaired {
                opener: false,
                closer: false,
            };
        }

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
        opener_delimiter: &LinkDelimiter,
        closer_delimiter: &LinkDelimiter,
        internal_tokens: &[InlineToken],
    ) -> ProcessDelimiterPairResult {
        let children_tokens = self.api.resolve_internal_tokens(
            internal_tokens,
            opener_delimiter.end_index,
            closer_delimiter.start_index,
        );

        let token = r#match::create_link_token(opener_delimiter, closer_delimiter, children_tokens);

        ProcessDelimiterPairResult {
            tokens: vec![token],
            remain_opener_delimiter: None,
            remain_closer_delimiter: None,
        }
    }

    fn lookup_delimiter(&self, delimiter: &TokenDelimiter) -> Option<LinkDelimiter> {
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

impl<'a> MatchInlineHook<'a> for LinkMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();

        let mut finder = LinkMatchHook::find_delimiter(self);
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

        LinkMatchHook::is_delimiter_pair(
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

        LinkMatchHook::process_delimiter_pair(
            self,
            &opener_delimiter,
            &closer_delimiter,
            internal_tokens,
        )
    }
}

pub struct LinkParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> LinkParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for LinkParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_link_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for LinkTokenizer {
    fn r#match<'b>(
        &'b self,
        api: &'b dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'b> + 'b> {
        Box::new(LinkMatchHook::new(api))
    }

    fn parse<'b>(&'b self, api: &'b dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'b> {
        Box::new(LinkParseHook::new(api))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::{Node, TEXT_TYPE};
    use yozora_character::{
        calc_escaped_string_from_node_points, create_node_point_generator, NodePoint,
    };

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
                TEXT_TYPE,
                (start_index, end_index),
            )]
        }
    }

    struct DummyParseApi {
        node_points: Vec<NodePoint>,
    }

    impl ParseInlinePhaseApi for DummyParseApi {
        fn should_reserve_position(&self) -> bool {
            false
        }

        fn calc_position(&self, _interval: NodeInterval) -> yozora_ast::Position {
            panic!("calc_position should not be called in this test")
        }

        fn format_url(&self, url: &str) -> String {
            url.to_string()
        }

        fn get_node_points(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn parse_inline_tokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node> {
            let Some(tokens) = tokens else {
                return Vec::new();
            };
            tokens
                .iter()
                .map(|token| {
                    Node::Text(yozora_ast::Text {
                        position: None,
                        value: calc_escaped_string_from_node_points(
                            &self.node_points,
                            token.start_index,
                            token.end_index,
                            false,
                        ),
                    })
                })
                .collect()
        }
    }

    fn create_single_link_token(input: &str) -> (InlineToken, Vec<NodePoint>) {
        let tokenizer = LinkTokenizer::default();
        let node_points = create_node_point_generator(input)
            .pop()
            .expect("expected node points");
        let match_api = DummyMatchApi {
            node_points: node_points.clone(),
        };

        let hook = tokenizer.r#match(&match_api);
        let mut find_delimiter = hook.find_delimiter();
        let opener = find_delimiter
            .next((0, match_api.get_block_end_index()))
            .expect("expected opener");
        let closer = find_delimiter
            .next((opener.end_index, match_api.get_block_end_index()))
            .expect("expected closer");

        let result = hook.process_delimiter_pair(&opener, &closer, &[]);
        let token = result
            .tokens
            .into_iter()
            .next()
            .expect("expected link token");
        (token, node_points)
    }

    #[test]
    fn match_should_capture_child_tokens_for_link_label() {
        let (token, node_points) = create_single_link_token("[link](/url)");
        token
            .data_as::<LinkTokenData>()
            .expect("expected link token data");

        assert_eq!(token.children.len(), 1);
        let child = &token.children[0];
        assert_eq!(
            calc_escaped_string_from_node_points(
                &node_points,
                child.start_index,
                child.end_index,
                false,
            ),
            "link"
        );
    }

    #[test]
    fn typed_hook_preserves_destination_and_title() {
        let node_points = create_node_point_generator("[foo](/uri \"title\")")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };
        let hook = LinkMatchHook::new(&api);
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
                start_index: 6,
                end_index: 10,
            })
        );
        assert_eq!(
            closer.title_content,
            Some(NodeInterval {
                start_index: 11,
                end_index: 18,
            })
        );
    }

    #[test]
    fn parse_should_preserve_link_title_and_children() {
        let tokenizer = LinkTokenizer::default();
        let (token, node_points) = create_single_link_token("[foo](/uri \"title\")");
        let parse_api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&parse_api);
        let nodes = parse_hook.parse(&[token]);

        let Some(Node::Link(link)) = nodes.first() else {
            panic!("expected link node");
        };
        assert_eq!(link.url, "/uri");
        assert_eq!(link.title.as_deref(), Some("title"));
        assert_eq!(link.children.len(), 1);
    }
}
