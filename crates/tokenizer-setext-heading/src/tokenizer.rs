use yozora_ast::{Heading, Node, Point, Position, HEADING_TYPE};
use yozora_character::{is_whitespace_character, AsciiCodePoint, VirtualCodePoint};
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

pub const SETEXT_HEADING_TOKENIZER_NAME: &str = "@yozora/tokenizer-setext-heading";

#[derive(Debug, Clone)]
pub struct SetextHeadingTokenizer {
    meta: TokenizerMeta,
}

impl Default for SetextHeadingTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: SETEXT_HEADING_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 7,
            },
        }
    }
}

impl Tokenizer for SetextHeadingTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for SetextHeadingTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_setext_heading_token(lines)?;
        Some(parse::parse_setext_heading_token(token))
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

#[derive(Debug, Clone)]
struct TokenData {
    marker: i32,
    lines: Vec<PhrasingContentLine>,
}

impl EngineTokenizer for SetextHeadingTokenizer {
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

struct SetextHeadingMatchHook<'a> {
    api: &'a dyn EngineMatchBlockPhaseApi,
}

impl MatchBlockHook for SetextHeadingMatchHook<'_> {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        _line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        None
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        if line.count_of_precede_spaces >= 4 || line.first_non_whitespace_index >= line.end_index {
            return None;
        }

        let node_points = line.node_points.as_ref();
        let mut marker: Option<i32> = None;
        let mut has_potential_internal_space = false;

        for i in line.first_non_whitespace_index..line.end_index {
            let code_point = node_points[i].code_point;
            if code_point == VirtualCodePoint::LineEnd as i32 {
                break;
            }

            if is_whitespace_character(code_point) {
                has_potential_internal_space = true;
                continue;
            }

            if has_potential_internal_space
                || (code_point != AsciiCodePoint::EQUALS_SIGN as i32
                    && code_point != AsciiCodePoint::MINUS_SIGN as i32)
                || marker.is_some_and(|m| m != code_point)
            {
                marker = None;
                break;
            }

            marker = Some(code_point);
        }

        let marker = marker?;
        let lines = self.api.extract_phrasing_lines(prev_sibling_token)?;
        let first_line = lines.first()?;

        let token = BlockToken::new("", HEADING_TYPE, calc_spanning_position(first_line, line))
            .with_data(TokenData { marker, lines });

        Some(EatAndInterruptPreviousSiblingResult {
            token,
            next_index: line.end_index,
            saturated: true,
            remaining_sibling: RemainingSibling::None,
        })
    }
}

struct SetextHeadingParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for SetextHeadingParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let depth = if data.marker == AsciiCodePoint::EQUALS_SIGN as i32 {
                1
            } else {
                2
            };

            let contents = merge_and_strip_content_lines(&data.lines);
            let children = self.api.process_inlines(&contents);
            nodes.push(Node::Heading(Heading {
                position: if self.api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                identifier: None,
                depth,
                children,
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for SetextHeadingTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(SetextHeadingMatchHook { api })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(SetextHeadingParseHook { api })
    }
}

fn merge_and_strip_content_lines(
    lines: &[PhrasingContentLine],
) -> Vec<yozora_character::NodePoint> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::new();
    let last_index = lines.len() - 1;

    for line in &lines[..last_index] {
        if line.first_non_whitespace_index >= line.end_index {
            continue;
        }
        merged
            .extend_from_slice(&line.node_points[line.first_non_whitespace_index..line.end_index]);
    }

    let last = &lines[last_index];
    if last.first_non_whitespace_index >= last.end_index {
        return merged;
    }

    let node_points = last.node_points.as_ref();
    let mut right = last.end_index;
    while right > last.first_non_whitespace_index {
        let idx = right - 1;
        if !is_whitespace_character(node_points[idx].code_point) {
            break;
        }
        right -= 1;
    }

    if right > last.first_non_whitespace_index {
        merged.extend_from_slice(&node_points[last.first_non_whitespace_index..right]);
    }

    merged
}

fn calc_spanning_position(
    first: &PhrasingContentLine,
    last: &PhrasingContentLine,
) -> Option<Position> {
    if first.start_index >= first.end_index || last.start_index >= last.end_index {
        return None;
    }

    let start = first.node_points[first.start_index];
    let end = last.node_points[last.end_index - 1];
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
