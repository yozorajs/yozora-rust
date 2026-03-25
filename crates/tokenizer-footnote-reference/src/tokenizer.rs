use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use std::collections::HashSet;
#[cfg(test)]
use yozora_character::{create_node_point_generator, NodePoint};
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::{parse, r#match};

pub const FOOTNOTE_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-reference";

#[derive(Debug, Clone)]
pub struct FootnoteReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteReferenceTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl FootnoteReferenceTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| FOOTNOTE_REFERENCE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for FootnoteReferenceTokenizer {
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

struct FootnoteReferenceMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
}

impl<'a> MatchInlineHook<'a> for FootnoteReferenceMatchHook<'a> {
    fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let api = self.api;

        Box::new(genFindDelimiter(|start_index, end_index| {
            let entry = r#match::find_delimiter_entry(api, start_index, end_index)?;
            Some(entry.delimiter)
        }))
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        r#match::process_single_delimiter(self.api, delimiter)
    }
}

struct FootnoteReferenceParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for FootnoteReferenceParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_footnote_reference_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for FootnoteReferenceTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(FootnoteReferenceMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(FootnoteReferenceParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::Node;

    struct DummyInlineApi {
        node_points: Vec<NodePoint>,
        footnotes: HashSet<String>,
    }

    impl DummyInlineApi {
        fn from(input: &str, footnotes: &[&str]) -> Self {
            let node_points = create_node_point_generator(input)
                .pop()
                .expect("expected node points");

            Self {
                node_points,
                footnotes: footnotes.iter().map(|v| v.to_string()).collect(),
            }
        }
    }

    impl MatchInlinePhaseApi for DummyInlineApi {
        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, identifier: &str) -> bool {
            self.footnotes.contains(identifier)
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
            higher_priority_tokens: &[InlineToken],
            _start_index: usize,
            _end_index: usize,
        ) -> Vec<InlineToken> {
            higher_priority_tokens.to_vec()
        }
    }

    impl ParseInlinePhaseApi for DummyInlineApi {
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

        fn hasFootnoteDefinition(&self, identifier: &str) -> bool {
            self.footnotes.contains(identifier)
        }

        fn parseInlineTokens(&self, _tokens: Option<&[InlineToken]>) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn match_should_filter_out_unknown_footnote() {
        let tokenizer = FootnoteReferenceTokenizer::default();
        let api = DummyInlineApi::from("[^missing]", &[]);

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.findDelimiter();
        let delimiter = find_delimiter
            .next((0, api.getBlockEndIndex()))
            .expect("expected delimiter");

        let tokens = hook.processSingleDelimiter(&delimiter);
        assert!(tokens.is_empty(), "unexpected tokens: {tokens:?}");
    }

    #[test]
    fn parse_should_build_footnote_reference_node() {
        let tokenizer = FootnoteReferenceTokenizer::default();
        let api = DummyInlineApi::from("[^note]", &["note"]);

        let match_hook = tokenizer.r#match(&api);
        let mut find_delimiter = match_hook.findDelimiter();
        let delimiter = find_delimiter
            .next((0, api.getBlockEndIndex()))
            .expect("expected delimiter");
        let tokens = match_hook.processSingleDelimiter(&delimiter);
        assert_eq!(tokens.len(), 1);

        let parse_hook = tokenizer.parse(&api);
        let nodes = parse_hook.parse(&tokens);
        assert_eq!(nodes.len(), 1);

        let Node::FootnoteReference(node) = &nodes[0] else {
            panic!("expected footnote reference node");
        };

        assert_eq!(node.identifier, "note");
        assert_eq!(node.label, "note");
    }
}
