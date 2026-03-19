use yozora_ast::{Heading, Node, Point, Position, HEADING_TYPE};
use yozora_character::{
    calc_trim_boundary_of_code_points, is_space_character, is_whitespace_character, AsciiCodePoint,
};
use yozora_core_tokenizer::engine::{
    BlockToken, EatOpenerResult, EngineBlockTokenizer, EngineTokenizer, MatchBlockHook,
    MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const HEADING_TOKENIZER_NAME: &str = "@yozora/tokenizer-heading";

#[derive(Debug, Clone)]
pub struct HeadingTokenizer {
    meta: TokenizerMeta,
}

impl Default for HeadingTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: HEADING_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 8,
            },
        }
    }
}

impl Tokenizer for HeadingTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for HeadingTokenizer {
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

    fn tokenize_block(&self, input: &str, _position: Option<yozora_ast::Position>) -> Option<Node> {
        let token = r#match::match_heading_token(input)?;
        Some(parse::parse_heading_token(token))
    }
}

#[derive(Debug, Clone)]
struct HeadingTokenData {
    depth: u8,
    line: PhrasingContentLine,
}

impl EngineTokenizer for HeadingTokenizer {
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

struct HeadingMatchHook;

impl MatchBlockHook for HeadingMatchHook {
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

        let first_non_whitespace_index = line.first_non_whitespace_index;
        if first_non_whitespace_index >= line.end_index {
            return None;
        }

        if line.node_points[first_non_whitespace_index].code_point
            != AsciiCodePoint::NUMBER_SIGN as i32
        {
            return None;
        }

        let mut i = first_non_whitespace_index + 1;
        while i < line.end_index
            && line.node_points[i].code_point == AsciiCodePoint::NUMBER_SIGN as i32
        {
            i += 1;
        }

        let depth = i - first_non_whitespace_index;
        if depth == 0 || depth > 6 {
            return None;
        }

        if i + 1 < line.end_index && !is_space_character(line.node_points[i].code_point) {
            return None;
        }

        let token = BlockToken::new("", HEADING_TYPE, calc_line_position(line)).with_data(
            HeadingTokenData {
                depth: depth as u8,
                line: line.clone(),
            },
        );

        Some(EatOpenerResult {
            token,
            next_index: line.end_index,
            saturated: true,
        })
    }
}

struct HeadingParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for HeadingParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<HeadingTokenData>() else {
                continue;
            };

            let line = &data.line;
            let node_points = line.node_points.as_ref();

            let (left_index, mut right_index) = calc_trim_boundary_of_code_points(
                node_points,
                line.first_non_whitespace_index + data.depth as usize,
                line.end_index,
            );

            let mut close_char_count = 0usize;
            let mut j = right_index;
            while j > left_index {
                let idx = j - 1;
                if node_points[idx].code_point != AsciiCodePoint::NUMBER_SIGN as i32 {
                    break;
                }
                close_char_count += 1;
                j -= 1;
            }

            if close_char_count > 0 {
                let mut space_count = 0usize;
                let mut k = right_index - close_char_count;
                while k > left_index {
                    let idx = k - 1;
                    if !is_whitespace_character(node_points[idx].code_point) {
                        break;
                    }
                    space_count += 1;
                    k -= 1;
                }

                if space_count > 0 || k == left_index {
                    right_index -= close_char_count + space_count;
                }
            }

            let inline_children = if left_index < right_index {
                self.api
                    .process_inlines(&node_points[left_index..right_index])
            } else {
                Vec::new()
            };

            nodes.push(Node::Heading(Heading {
                position: if self.api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                identifier: None,
                depth: data.depth,
                children: inline_children,
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for HeadingTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(HeadingMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(HeadingParseHook { api })
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
