use std::sync::Arc;

use yozora_ast::Node;
use yozora_ast::{Point, Position, PARAGRAPH_TYPE};
use yozora_core_tokenizer::engine::{
    BlockToken, EatLazyContinuationTextResult, EatOpenerResult, EngineBlockTokenizer,
    EngineTokenizer, MatchBlockHook, MatchBlockPhaseApi as EngineMatchBlockPhaseApi,
    ParseBlockHook, ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine,
    TokenizerType,
};
use yozora_core_tokenizer::{
    BlockFallbackTokenizer, BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi,
    ParseBlockPhaseApi, Tokenizer, TokenizerKind, TokenizerMeta,
};

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
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for ParagraphTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        r#match::match_paragraph_block_lines(lines).map(|_| unreachable!())
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
        _api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        self.tokenize_block_lines(lines, position)
    }
}

impl BlockFallbackTokenizer for ParagraphTokenizer {
    fn build_block(
        &self,
        inline_children: Vec<Node>,
        position: Option<yozora_ast::Position>,
    ) -> Node {
        parse::parse_paragraph_node(inline_children, position)
    }

    fn build_block_with_api(
        &self,
        inline_children: Vec<Node>,
        position: Option<yozora_ast::Position>,
        _api: &dyn ParseBlockPhaseApi,
    ) -> Node {
        self.build_block(inline_children, position)
    }
}

#[derive(Debug, Clone)]
struct ParagraphTokenData {
    lines: Vec<PhrasingContentLine>,
}

impl EngineTokenizer for ParagraphTokenizer {
    fn tokenizer_type(&self) -> TokenizerType {
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

impl ParagraphMatchHook {
    fn create_token(line: &PhrasingContentLine) -> Option<BlockToken> {
        if line.start_index >= line.end_index {
            return None;
        }

        let start = line.node_points[line.start_index];
        let end = line.node_points[line.end_index - 1];
        let position = Position {
            start: Point {
                line: start.line,
                column: start.column,
                offset: Some(start.offset),
            },
            end: Point {
                line: end.line,
                column: end.column + 1,
                offset: Some(end.offset + 1),
            },
            indent: None,
        };

        let mut token = BlockToken::new("", PARAGRAPH_TYPE, Some(position));
        token.data = Arc::new(ParagraphTokenData {
            lines: vec![line.clone()],
        });
        Some(token)
    }

    fn get_lines(token: &BlockToken) -> Vec<PhrasingContentLine> {
        token
            .data_as::<ParagraphTokenData>()
            .map(|data| data.lines.clone())
            .unwrap_or_default()
    }

    fn set_lines(token: &mut BlockToken, lines: Vec<PhrasingContentLine>) {
        token.data = Arc::new(ParagraphTokenData { lines });
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
        let token = Self::create_token(line)?;
        Some(EatOpenerResult {
            token,
            next_index: line.end_index,
            saturated: false,
        })
    }

    fn eat_lazy_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatLazyContinuationTextResult {
        if line.start_index >= line.end_index {
            return EatLazyContinuationTextResult::NotMatched;
        }

        let mut lines = Self::get_lines(token);
        lines.push(line.clone());
        Self::set_lines(token, lines);

        if let Some(position) = token.position.as_mut() {
            let end = line.node_points[line.end_index - 1];
            position.end = Point {
                line: end.line,
                column: end.column + 1,
                offset: Some(end.offset + 1),
            };
        }

        EatLazyContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

struct ParagraphParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for ParagraphParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<ParagraphTokenData>() else {
                continue;
            };

            let mut node_points = Vec::new();
            for line in &data.lines {
                if line.start_index >= line.end_index {
                    continue;
                }
                node_points.extend_from_slice(&line.node_points[line.start_index..line.end_index]);
            }

            let inline_children = self.api.process_inlines(&node_points);
            let position = if self.api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            };
            nodes.push(parse::parse_paragraph_node(inline_children, position));
        }

        nodes
    }
}

impl EngineBlockTokenizer for ParagraphTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(ParagraphMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(ParagraphParseHook { api })
    }

    fn extract_phrasing_content_lines(
        &self,
        token: &BlockToken,
    ) -> Option<Vec<PhrasingContentLine>> {
        token
            .data_as::<ParagraphTokenData>()
            .map(|data| data.lines.clone())
    }

    fn build_block_token(
        &self,
        lines: &[PhrasingContentLine],
        original_token: &BlockToken,
    ) -> Option<BlockToken> {
        let first = lines.first()?;
        if first.start_index >= first.end_index {
            return None;
        }

        let start = first.node_points[first.start_index];
        let mut end_point = Point {
            line: start.line,
            column: start.column + 1,
            offset: Some(start.offset + 1),
        };

        if let Some(last) = lines.last() {
            if last.start_index < last.end_index {
                let end = last.node_points[last.end_index - 1];
                end_point = Point {
                    line: end.line,
                    column: end.column + 1,
                    offset: Some(end.offset + 1),
                };
            }
        }

        let mut token = BlockToken::new(
            original_token.tokenizer.clone(),
            original_token.node_type,
            Some(Position {
                start: Point {
                    line: start.line,
                    column: start.column,
                    offset: Some(start.offset),
                },
                end: end_point,
                indent: None,
            }),
        );
        token.data = Arc::new(ParagraphTokenData {
            lines: lines.to_vec(),
        });
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyBlockApi;

    impl ParseBlockPhaseApi for DummyBlockApi {
        fn should_reserve_position(&self) -> bool {
            false
        }

        fn format_url(&self, url: &str) -> String {
            url.to_string()
        }
    }

    #[test]
    fn phase_fallback_should_build_paragraph() {
        let tokenizer = ParagraphTokenizer::default();
        let api = DummyBlockApi;
        let children = vec![Node::Text(yozora_ast::Text {
            position: None,
            value: "hello".to_string(),
        })];

        let node = tokenizer.build_block_with_api(children, None, &api);
        let Node::Paragraph(paragraph) = node else {
            panic!("expected paragraph node");
        };
        assert_eq!(paragraph.children.len(), 1);
    }
}
