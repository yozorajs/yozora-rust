use std::cell::RefCell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const LINK_TOKENIZER_NAME: &str = "@yozora/tokenizer-link";

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
                name: options.name.unwrap_or_else(|| LINK_TOKENIZER_NAME.to_string()),
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

struct LinkMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    source: String,
    char_starts: Vec<usize>,
    block_start_index: usize,
    block_end_index: usize,
    delimiters: Rc<RefCell<Vec<r#match::DelimiterEntry>>>,
}

impl<'a> LinkMatchHook<'a> {
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

    fn lookup_data(&self, delimiter: &TokenDelimiter) -> Option<r#match::LinkDelimiterData> {
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

impl<'a> MatchInlineHook<'a> for LinkMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();

        let source = self.source.clone();
        let char_starts = self.char_starts.clone();
        let api = self.api;
        let block_start_index = self.block_start_index;
        let block_end_index = self.block_end_index;
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(genFindDelimiter(
            move |start_index, end_index| {
                let entry = r#match::find_link_delimiter_entry(
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
            },
        ))
    }

    fn isDelimiterPair(
        &self,
        opener_delimiter: &TokenDelimiter,
        closer_delimiter: &TokenDelimiter,
        internal_tokens: &[InlineToken],
    ) -> IsDelimiterPairResult {
        // Links may not contain other links, at any level of nesting.
        if internal_tokens.iter().any(is_link_token) {
            return IsDelimiterPairResult::NotPaired {
                opener: false,
                closer: false,
            };
        }

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

        let token = r#match::create_link_token(
            self.api.getNodePoints(),
            opener_delimiter,
            closer_delimiter,
            data.url,
            data.title,
            children_tokens,
        );

        ProcessDelimiterPairResult {
            tokens: vec![token],
            remainOpenerDelimiter: None,
            remainCloserDelimiter: None,
        }
    }
}

struct LinkParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
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

    fn parse<'b>(
        &'b self,
        api: &'b dyn ParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'b> {
        Box::new(LinkParseHook { api })
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
        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn getNodePoints(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn getBlockStartIndex(&self) -> usize {
            0
        }

        fn getBlockEndIndex(&self) -> usize {
            self.node_points.len()
        }

        fn resolveFallbackTokens(
            &self,
            tokens: &[InlineToken],
            _token_start_index: usize,
            _token_end_index: usize,
        ) -> Vec<InlineToken> {
            tokens.to_vec()
        }

        fn resolveInternalTokens(
            &self,
            _higher_priority_tokens: &[InlineToken],
            start_index: usize,
            end_index: usize,
        ) -> Vec<InlineToken> {
            vec![InlineToken::new("text", TEXT_TYPE, (start_index, end_index))]
        }
    }

    struct DummyParseApi {
        node_points: Vec<NodePoint>,
    }

    impl ParseInlinePhaseApi for DummyParseApi {
        fn shouldReservePosition(&self) -> bool {
            false
        }

        fn calcPosition(&self, _interval: NodeInterval) -> yozora_ast::Position {
            panic!("calcPosition should not be called in this test")
        }

        fn formatUrl(&self, url: &str) -> String {
            url.to_string()
        }

        fn getNodePoints(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn parseInlineTokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node> {
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
        let mut find_delimiter = hook.findDelimiter();
        let opener = find_delimiter
            .next((0, match_api.getBlockEndIndex()))
            .expect("expected opener");
        let closer = find_delimiter
            .next((opener.end_index, match_api.getBlockEndIndex()))
            .expect("expected closer");

        let result = hook.processDelimiterPair(&opener, &closer, &[]);
        let token = result.tokens.into_iter().next().expect("expected link token");
        (token, node_points)
    }

    #[test]
    fn match_should_capture_child_tokens_for_link_label() {
        let (token, node_points) = create_single_link_token("[link](/url)");
        let data = token
            .data_as::<parse::LinkTokenData>()
            .expect("expected link token data");

        assert_eq!(data.children_tokens.len(), 1);
        let child = &data.children_tokens[0];
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
