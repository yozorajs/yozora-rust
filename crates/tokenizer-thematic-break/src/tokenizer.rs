use yozora_ast::{Node, Point, Position, ThematicBreak, THEMATIC_BREAK_TYPE};
use yozora_character::{is_whitespace_character, AsciiCodePoint};
use yozora_core_tokenizer::engine::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatOpenerResult, EngineBlockTokenizer,
    EngineTokenizer, MatchBlockHook, MatchBlockPhaseApi as EngineMatchBlockPhaseApi,
    ParseBlockHook, ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine,
    RemainingSibling, TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const THEMATIC_BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-thematic-break";

#[derive(Debug, Clone)]
pub struct ThematicBreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for ThematicBreakTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: THEMATIC_BREAK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 7,
            },
        }
    }
}

impl Tokenizer for ThematicBreakTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for ThematicBreakTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        let node = self.tokenize_block(first, position)?;
        Some(BlockTokenizeResult {
            node,
            consumed_lines: 1,
        })
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
        _api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        self.tokenize_block_lines(lines, position)
    }

    fn tokenize_block(&self, input: &str, position: Option<yozora_ast::Position>) -> Option<Node> {
        let token = r#match::match_thematic_break_token(input)?;
        Some(parse::parse_thematic_break_token(token, position))
    }
}

impl EngineTokenizer for ThematicBreakTokenizer {
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

struct ThematicBreakMatchHook;

impl MatchBlockHook for ThematicBreakMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        if line.count_of_precede_spaces >= 4 {
            return None;
        }

        if line.first_non_whitespace_index + 2 >= line.end_index {
            return None;
        }

        let node_points = line.node_points.as_ref();
        let mut marker: Option<i32> = None;
        let mut count = 0usize;

        for i in line.first_non_whitespace_index..line.end_index {
            let code_point = node_points[i].code_point;
            if is_whitespace_character(code_point) {
                continue;
            }

            match code_point {
                x if x == AsciiCodePoint::MINUS_SIGN as i32
                    || x == AsciiCodePoint::UNDERSCORE as i32
                    || x == AsciiCodePoint::ASTERISK as i32 =>
                {
                    if let Some(existed) = marker {
                        if existed != x {
                            return None;
                        }
                    } else {
                        marker = Some(x);
                    }
                    count += 1;
                }
                _ => return None,
            }
        }

        if count < 3 {
            return None;
        }

        Some(EatOpenerResult {
            token: BlockToken::new("", THEMATIC_BREAK_TYPE, calc_line_position(line)),
            next_index: line.end_index,
            saturated: true,
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
}

struct ThematicBreakParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for ThematicBreakParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        tokens
            .iter()
            .map(|token| {
                Node::ThematicBreak(ThematicBreak {
                    position: if self.api.should_reserve_position() {
                        token.position.clone()
                    } else {
                        None
                    },
                })
            })
            .collect()
    }
}

impl EngineBlockTokenizer for ThematicBreakTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(ThematicBreakMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(ThematicBreakParseHook { api })
    }
}

fn calc_line_position(line: &PhrasingContentLine) -> Option<Position> {
    if line.start_index >= line.end_index {
        return None;
    }

    let start = line.node_points[line.start_index];
    let end = line.node_points[line.end_index - 1];

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
