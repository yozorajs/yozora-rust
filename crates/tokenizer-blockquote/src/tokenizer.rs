use yozora_ast::{Blockquote, Node, Point, Position, BLOCKQUOTE_TYPE};
use yozora_character::{is_space_character, AsciiCodePoint, VirtualCodePoint};
use yozora_core_tokenizer::engine::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    EngineBlockTokenizer, EngineTokenizer, MatchBlockHook,
    MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, RemainingSibling,
    TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const BLOCKQUOTE_TOKENIZER_NAME: &str = "@yozora/tokenizer-blockquote";

#[derive(Debug, Clone)]
pub struct BlockquoteTokenizer {
    meta: TokenizerMeta,
}

impl Default for BlockquoteTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: BLOCKQUOTE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 9,
            },
        }
    }
}

impl Tokenizer for BlockquoteTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for BlockquoteTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_blockquote_token(lines)?;
        Some(parse::parse_blockquote_token(token))
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

impl EngineTokenizer for BlockquoteTokenizer {
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

struct BlockquoteMatchHook;

impl MatchBlockHook for BlockquoteMatchHook {
    fn is_containing_block(&self) -> bool {
        true
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        if line.count_of_precede_spaces >= 4 {
            return None;
        }

        let first = line.first_non_whitespace_index;
        if first >= line.end_index {
            return None;
        }

        let marker = line.node_points[first].code_point;
        if marker != AsciiCodePoint::CLOSE_ANGLE as i32 {
            return None;
        }

        let mut next_index = first + 1;
        if next_index < line.end_index
            && is_space_character(line.node_points[next_index].code_point)
        {
            next_index += 1;
            if next_index < line.end_index
                && line.node_points[next_index].code_point == VirtualCodePoint::Space as i32
            {
                next_index += 1;
            }
        }

        let token = BlockToken::new(
            "",
            BLOCKQUOTE_TYPE,
            calc_segment_position(line, line.start_index, next_index),
        );

        Some(EatOpenerResult {
            token,
            next_index,
            saturated: false,
        })
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        let opener = self.eat_opener(line, prev_sibling_token)?;

        Some(EatAndInterruptPreviousSiblingResult {
            token: opener.token,
            next_index: opener.next_index,
            saturated: opener.saturated,
            remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
        })
    }

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        _token: &mut BlockToken,
        parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        let first = line.first_non_whitespace_index;
        let marker = line
            .node_points
            .get(first)
            .map(|point| point.code_point)
            .unwrap_or_default();

        if line.count_of_precede_spaces >= 4
            || first >= line.end_index
            || marker != AsciiCodePoint::CLOSE_ANGLE as i32
        {
            if parent_token.node_type == BLOCKQUOTE_TYPE {
                return EatContinuationTextResult::Opening {
                    next_index: line.start_index,
                };
            }

            return EatContinuationTextResult::NotMatched;
        }

        let mut next_index = first + 1;
        if next_index < line.end_index
            && is_space_character(line.node_points[next_index].code_point)
        {
            next_index += 1;
        }

        EatContinuationTextResult::Opening { next_index }
    }
}

struct BlockquoteParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for BlockquoteParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let children = self.api.parse_block_tokens(&token.children);
            let position = if self.api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            };

            nodes.push(Node::Blockquote(Blockquote { position, children }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for BlockquoteTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(BlockquoteMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(BlockquoteParseHook { api })
    }
}

fn calc_segment_position(
    line: &PhrasingContentLine,
    start_index: usize,
    end_index: usize,
) -> Option<Position> {
    if start_index >= end_index {
        return None;
    }

    let start = line.node_points[start_index];
    let end = line.node_points[end_index - 1];

    Some(Position {
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
    })
}
