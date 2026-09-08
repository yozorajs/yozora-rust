use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{
    LinkReferenceDelimiter, LinkReferenceProcessDelimiterPairResult, LINK_REFERENCE_TOKENIZER_NAME,
};
use crate::{parse, r#match};

type DelimiterKey = (usize, usize, u8);
type DelimiterMap = Rc<RefCell<HashMap<DelimiterKey, LinkReferenceDelimiter>>>;

#[derive(Debug, Clone)]
pub struct LinkReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for LinkReferenceTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl LinkReferenceTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| LINK_REFERENCE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::LINKS),
            },
        }
    }
}

impl Tokenizer for LinkReferenceTokenizer {
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

pub struct LinkReferenceDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl LinkReferenceDelimiterGenerator<'_> {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<LinkReferenceDelimiter> {
        r#match::find_link_reference_delimiter_entry(
            self.api.get_node_points(),
            range_index.0,
            range_index.1,
        )
    }
}

pub struct LinkReferenceMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    delimiters: DelimiterMap,
}

impl<'a> LinkReferenceMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self {
            api,
            delimiters: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn find_delimiter(&self) -> LinkReferenceDelimiterGenerator<'a> {
        LinkReferenceDelimiterGenerator { api: self.api }
    }

    pub fn is_delimiter_pair(
        &self,
        opener_delimiter: &LinkReferenceDelimiter,
        closer_delimiter: &LinkReferenceDelimiter,
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
            0 => {
                let Some(bracket) = closer_delimiter.brackets.first() else {
                    return IsDelimiterPairResult::NotPaired {
                        opener: false,
                        closer: false,
                    };
                };

                let Some(identifier) = &bracket.identifier else {
                    return IsDelimiterPairResult::NotPaired {
                        opener: false,
                        closer: false,
                    };
                };

                if !self.api.has_definition(identifier) {
                    return IsDelimiterPairResult::NotPaired {
                        opener: false,
                        closer: false,
                    };
                }

                IsDelimiterPairResult::Paired
            }
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
        opener_delimiter: &LinkReferenceDelimiter,
        closer_delimiter: &LinkReferenceDelimiter,
        internal_tokens: &[InlineToken],
    ) -> LinkReferenceProcessDelimiterPairResult {
        let mut tokens = self.process_single_delimiter(opener_delimiter);

        let Some((first, remains)) = closer_delimiter.brackets.split_first() else {
            return LinkReferenceProcessDelimiterPairResult {
                tokens,
                remain_opener_delimiter: None,
                remain_closer_delimiter: None,
            };
        };

        let (Some(label), Some(identifier)) = (first.label.clone(), first.identifier.clone())
        else {
            return LinkReferenceProcessDelimiterPairResult {
                tokens,
                remain_opener_delimiter: None,
                remain_closer_delimiter: None,
            };
        };

        let children_tokens = self.api.resolve_internal_tokens(
            internal_tokens,
            opener_delimiter.end_index,
            closer_delimiter.start_index,
        );

        tokens.push(r#match::create_reference_token(
            opener_delimiter.end_index.saturating_sub(1),
            first.end_index,
            yozora_ast::ReferenceType::Full,
            label,
            identifier,
            children_tokens,
        ));

        let remain_type = if closer_delimiter.delimiter_type == DelimiterType::Both {
            DelimiterType::Opener
        } else {
            DelimiterType::Full
        };

        let remain_closer_delimiter = LinkReferenceDelimiter {
            delimiter_type: remain_type,
            start_index: first.end_index,
            end_index: closer_delimiter.end_index,
            thickness: closer_delimiter.end_index.saturating_sub(first.end_index),
            original_thickness: closer_delimiter.end_index.saturating_sub(first.end_index),
            brackets: remains.to_vec(),
        };

        LinkReferenceProcessDelimiterPairResult {
            tokens,
            remain_opener_delimiter: None,
            remain_closer_delimiter: Some(remain_closer_delimiter),
        }
    }

    pub fn process_single_delimiter(&self, delimiter: &LinkReferenceDelimiter) -> Vec<InlineToken> {
        r#match::process_single_delimiter(self.api, delimiter)
    }

    fn register_delimiter(&self, delimiter: LinkReferenceDelimiter) {
        let key = delimiter_key_from_typed(&delimiter);
        self.delimiters.borrow_mut().insert(key, delimiter);
    }

    fn lookup_delimiter(&self, delimiter: &TokenDelimiter) -> Option<LinkReferenceDelimiter> {
        self.delimiters
            .borrow()
            .get(&delimiter_key_from_core(delimiter))
            .cloned()
    }
}

impl<'a> MatchInlineHook<'a> for LinkReferenceMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();

        let mut finder = LinkReferenceMatchHook::find_delimiter(self);
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(gen_find_delimiter(move |start_index, end_index| {
            let delimiter = finder.next((start_index, end_index))?;
            let core_delimiter = delimiter.to_core();
            let key = delimiter_key_from_typed(&delimiter);
            delimiters.borrow_mut().insert(key, delimiter);
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

        LinkReferenceMatchHook::is_delimiter_pair(
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

        let result = LinkReferenceMatchHook::process_delimiter_pair(
            self,
            &opener_delimiter,
            &closer_delimiter,
            internal_tokens,
        );

        let remain_opener_delimiter = result.remain_opener_delimiter.map(|delimiter| {
            let core_delimiter = delimiter.to_core();
            self.register_delimiter(delimiter);
            core_delimiter
        });
        let remain_closer_delimiter = result.remain_closer_delimiter.map(|delimiter| {
            let core_delimiter = delimiter.to_core();
            self.register_delimiter(delimiter);
            core_delimiter
        });

        ProcessDelimiterPairResult {
            tokens: result.tokens,
            remain_opener_delimiter,
            remain_closer_delimiter,
        }
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(delimiter) = self.lookup_delimiter(delimiter) else {
            return Vec::new();
        };

        LinkReferenceMatchHook::process_single_delimiter(self, &delimiter)
    }
}

pub struct LinkReferenceParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> LinkReferenceParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for LinkReferenceParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_link_reference_tokens(tokens, self.api).resolve(self.api)
    }
}

impl InlineTokenizer for LinkReferenceTokenizer {
    fn parse_deferred<'a>(
        &'a self,
        tokens: &'a [InlineToken],
        api: &'a dyn ParseInlinePhaseApi,
    ) -> Option<ParseInlineHookResult<'a>> {
        Some(parse::parse_link_reference_tokens(tokens, api))
    }

    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(LinkReferenceMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(LinkReferenceParseHook::new(api))
    }
}

fn delimiter_key_from_core(delimiter: &TokenDelimiter) -> DelimiterKey {
    let delimiter_type = match delimiter.delimiter_type {
        DelimiterType::Opener => 0,
        DelimiterType::Closer => 1,
        DelimiterType::Both => 2,
        DelimiterType::Full => 3,
    };
    (delimiter.start_index, delimiter.end_index, delimiter_type)
}

fn delimiter_key_from_typed(delimiter: &LinkReferenceDelimiter) -> DelimiterKey {
    delimiter_key_from_core(&delimiter.to_core())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use yozora_ast::{Node, ReferenceType, TEXT_TYPE};
    use yozora_character::{
        calc_escaped_string_from_node_points, create_node_point_generator, NodePoint,
    };

    use super::*;

    struct DummyMatchApi {
        definitions: HashSet<String>,
        node_points: Vec<NodePoint>,
    }

    impl DummyMatchApi {
        fn from(definitions: &[&str], node_points: Vec<NodePoint>) -> Self {
            Self {
                definitions: definitions.iter().map(|v| v.to_string()).collect(),
                node_points,
            }
        }
    }

    impl MatchInlinePhaseApi for DummyMatchApi {
        fn has_definition(&self, identifier: &str) -> bool {
            self.definitions.contains(identifier)
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

    fn collect_single_token(
        input: &str,
        definitions: &[&str],
    ) -> Option<(InlineToken, Vec<NodePoint>)> {
        let tokenizer = LinkReferenceTokenizer::default();
        let node_points = create_node_point_generator(input)
            .pop()
            .expect("expected node points");
        let match_api = DummyMatchApi::from(definitions, node_points.clone());
        let hook = tokenizer.r#match(&match_api);

        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter.next((0, match_api.get_block_end_index()))?;
        let token = hook.process_single_delimiter(&delimiter).pop()?;
        Some((token, node_points))
    }

    #[test]
    fn should_skip_unknown_definition() {
        let result = collect_single_token("[foo][bar]", &[]);
        assert!(result.is_none());
    }

    #[test]
    fn typed_hook_preserves_reference_brackets() {
        let node_points = create_node_point_generator("[foo][]")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi::from(&["foo"], node_points);
        let hook = LinkReferenceMatchHook::new(&api);
        let mut finder = hook.find_delimiter();
        let delimiter = finder
            .next((0, api.get_block_end_index()))
            .expect("expected link reference delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.brackets.len(), 2);
        assert_eq!(delimiter.brackets[0].identifier.as_deref(), Some("foo"));
        assert_eq!(delimiter.brackets[1].identifier, None);
    }

    #[test]
    fn should_parse_full_reference_with_children() {
        let tokenizer = LinkReferenceTokenizer::default();
        let (token, node_points) =
            collect_single_token("[foo *bar*][ref]", &["ref"]).expect("should parse");
        let parse_api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&parse_api);
        let nodes = parse_hook.parse(&[token]);

        let Some(Node::LinkReference(link)) = nodes.first() else {
            panic!("expected linkReference");
        };
        assert_eq!(link.reference_type, ReferenceType::Full);
        assert_eq!(link.label, "ref");
        assert_eq!(link.children.len(), 1);
    }
}
