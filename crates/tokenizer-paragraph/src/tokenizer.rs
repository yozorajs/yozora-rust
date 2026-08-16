use yozora_core_tokenizer::*;

use crate::types::PARAGRAPH_TOKENIZER_NAME;
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct ParagraphTokenizer {
    meta: TokenizerMeta,
}

impl Default for ParagraphTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl ParagraphTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| PARAGRAPH_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::FALLBACK),
            },
        }
    }
}

impl Tokenizer for ParagraphTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        &self.meta.name
    }

    fn priority(&self) -> i32 {
        self.meta.priority
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ParagraphMatchHook;

impl ParagraphMatchHook {
    pub fn new() -> Self {
        Self
    }
}

impl MatchBlockHook for ParagraphMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        r#match::eat_opener(line)
    }

    fn eat_lazy_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatLazyContinuationTextResult {
        r#match::eat_lazy_continuation_text(line, token)
    }

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        r#match::eat_continuation_text(line, token)
    }
}

pub struct ParagraphParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl<'a> ParagraphParseHook<'a> {
    pub fn new(api: &'a dyn ParseBlockPhaseApi) -> Self {
        Self { api }
    }
}

impl ParseBlockHook for ParagraphParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(parse::parse_paragraph_tokens(tokens, self.api).into())
    }
}

impl BlockTokenizer for ParagraphTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(ParagraphMatchHook::new())
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(ParagraphParseHook::new(api))
    }

    fn extract_phrasing_content_lines(
        &self,
        token: &BlockToken,
    ) -> Option<Vec<PhrasingContentLine>> {
        r#match::extract_lines(token)
    }

    fn build_block_token(
        &self,
        lines: &[PhrasingContentLine],
        original_token: &BlockToken,
    ) -> Option<BlockToken> {
        r#match::build_block_token(lines, original_token)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;

    use super::*;
    use yozora_ast::{Node, Text, PARAGRAPH_TYPE};
    use yozora_character::{calc_string_from_node_points, create_node_point_generator, NodePoint};

    struct DummyBlockApi;

    impl ParseBlockPhaseApi for DummyBlockApi {
        fn should_reserve_position(&self) -> bool {
            false
        }

        fn format_url(&self, url: &str) -> String {
            url.to_string()
        }

        fn process_inlines(&self, node_points: &[NodePoint]) -> Vec<Node> {
            vec![Node::Text(Text {
                position: None,
                value: calc_string_from_node_points(node_points, 0, node_points.len(), false),
            })]
        }
    }

    struct TrackingBlockApi {
        pointer: Cell<*const NodePoint>,
        len: Cell<usize>,
    }

    impl ParseBlockPhaseApi for TrackingBlockApi {
        fn should_reserve_position(&self) -> bool {
            false
        }

        fn format_url(&self, url: &str) -> String {
            url.to_string()
        }

        fn process_inlines(&self, node_points: &[NodePoint]) -> Vec<Node> {
            self.pointer.set(node_points.as_ptr());
            self.len.set(node_points.len());
            vec![Node::Text(Text {
                position: None,
                value: calc_string_from_node_points(node_points, 0, node_points.len(), false),
            })]
        }
    }

    #[test]
    fn should_build_and_parse_paragraph_token() {
        let tokenizer = ParagraphTokenizer::default();
        let api = DummyBlockApi;

        let node_points = create_node_point_generator("hello")
            .pop()
            .expect("expected node points");
        let line = PhrasingContentLine {
            node_points: Arc::new(node_points),
            start_index: 0,
            end_index: 5,
            first_non_whitespace_index: 0,
            indent_width: 0,
            count_of_precede_spaces: 0,
        };

        let original_token = BlockToken::new(PARAGRAPH_TOKENIZER_NAME, PARAGRAPH_TYPE, None);
        let token = tokenizer
            .build_block_token(&[line], &original_token)
            .expect("expected block token");

        let parse_hook = tokenizer.parse(&api);
        let tokens = [token];
        let ParseBlockHookResult::Nodes(nodes) = parse_hook.parse(&tokens).unwrap() else {
            panic!("expected synchronous paragraph parse result");
        };
        assert_eq!(nodes.len(), 1);
        let Node::Paragraph(paragraph) = &nodes[0] else {
            panic!("expected paragraph node");
        };
        assert_eq!(paragraph.children.len(), 1);
    }

    #[test]
    fn exposes_lines_builds_nonempty_tokens_and_reuses_single_line_points() {
        let tokenizer = ParagraphTokenizer::default();
        let node_points = create_node_point_generator("hello, world!\nhello,")
            .pop()
            .expect("expected node points");
        let node_points = Arc::new(node_points);
        let line = PhrasingContentLine {
            node_points: Arc::clone(&node_points),
            start_index: 0,
            end_index: 14,
            first_non_whitespace_index: 0,
            indent_width: 0,
            count_of_precede_spaces: 0,
        };
        let original_token = BlockToken::new(PARAGRAPH_TOKENIZER_NAME, PARAGRAPH_TYPE, None);

        assert!(tokenizer.build_block_token(&[], &original_token).is_none());
        let token = tokenizer
            .build_block_token(std::slice::from_ref(&line), &original_token)
            .expect("expected paragraph token");
        let extracted = tokenizer
            .extract_phrasing_content_lines(&token)
            .expect("expected paragraph lines");
        assert_eq!(extracted.len(), 1);
        assert!(Arc::ptr_eq(&extracted[0].node_points, &node_points));

        let api = TrackingBlockApi {
            pointer: Cell::new(std::ptr::null()),
            len: Cell::new(0),
        };
        let parse_hook = tokenizer.parse(&api);
        let ParseBlockHookResult::Nodes(nodes) = parse_hook.parse(&[token]).unwrap() else {
            panic!("expected synchronous paragraph parse result");
        };
        assert_eq!(api.pointer.get(), node_points.as_ptr());
        assert_eq!(api.len.get(), 13);
        assert!(matches!(
            nodes.as_slice(),
            [Node::Paragraph(paragraph)]
                if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "hello, world!")
        ));
    }
}
