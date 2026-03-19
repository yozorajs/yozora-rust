use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const PARAGRAPH_TOKENIZER_NAME: &str = "@yozora/tokenizer-paragraph";

#[derive(Debug, Clone)]
pub struct ParagraphTokenizer {
    meta: TokenizerMeta,
}

impl Default for ParagraphTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: PARAGRAPH_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: -1,
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

struct ParagraphMatchHook;

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

struct ParagraphParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for ParagraphParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_paragraph_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for ParagraphTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(ParagraphMatchHook)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(ParagraphParseHook { api })
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
    use std::sync::Arc;

    use super::*;
    use yozora_ast::{Text, PARAGRAPH_TYPE};
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

        fn parse_block_tokens(&self, _tokens: &[BlockToken]) -> Vec<Node> {
            Vec::new()
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
            count_of_precede_spaces: 0,
        };

        let original_token = BlockToken::new(PARAGRAPH_TOKENIZER_NAME, PARAGRAPH_TYPE, None);
        let token = tokenizer
            .build_block_token(&[line], &original_token)
            .expect("expected block token");

        let parse_hook = tokenizer.parse(&api);
        let nodes = parse_hook.parse(&[token]);
        assert_eq!(nodes.len(), 1);
        let Node::Paragraph(paragraph) = &nodes[0] else {
            panic!("expected paragraph node");
        };
        assert_eq!(paragraph.children.len(), 1);
    }
}
