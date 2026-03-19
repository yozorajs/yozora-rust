use yozora_ast::{Math, Node, Point, Position, MATH_TYPE};
use yozora_character::{
    calc_string_from_node_points, calc_trim_boundary_of_code_points, is_space_character,
    AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::engine::{
    BlockToken, EatContinuationTextResult, EatOpenerResult, EngineBlockTokenizer, EngineTokenizer,
    MatchBlockHook, MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const MATH_TOKENIZER_NAME: &str = "@yozora/tokenizer-math";

#[derive(Debug, Clone)]
pub struct MathTokenizer {
    meta: TokenizerMeta,
}

impl Default for MathTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: MATH_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 11,
            },
        }
    }
}

impl Tokenizer for MathTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for MathTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_math_token(lines)?;
        Some(parse::parse_math_token(token))
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
    marker_count: usize,
    indent: usize,
    lines: Vec<PhrasingContentLine>,
}

impl EngineTokenizer for MathTokenizer {
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

struct MathMatchHook;

impl MatchBlockHook for MathMatchHook {
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
        if first_non_whitespace_index + 1 >= line.end_index {
            return None;
        }

        let node_points = line.node_points.as_ref();
        if node_points[first_non_whitespace_index].code_point != AsciiCodePoint::DOLLAR_SIGN as i32
        {
            return None;
        }

        let mut i = first_non_whitespace_index + 1;
        while i < line.end_index && node_points[i].code_point == AsciiCodePoint::DOLLAR_SIGN as i32
        {
            i += 1;
        }

        let marker_count = i - first_non_whitespace_index;
        if marker_count < 2 {
            return None;
        }

        let (left, right) = calc_trim_boundary_of_code_points(node_points, i, line.end_index);

        let token = BlockToken::new("", MATH_TYPE, calc_line_position(line)).with_data(TokenData {
            marker_count,
            indent: first_non_whitespace_index.saturating_sub(line.start_index),
            lines: Vec::new(),
        });

        if left < right {
            let mut j = right;
            while j > left && node_points[j - 1].code_point == AsciiCodePoint::DOLLAR_SIGN as i32 {
                j -= 1;
            }
            let count_of_trailing_marker = right - j;
            if count_of_trailing_marker != marker_count {
                return None;
            }

            let mut lines = Vec::new();
            lines.push(PhrasingContentLine {
                node_points: line.node_points.clone(),
                start_index: left,
                end_index: j,
                first_non_whitespace_index: left,
                count_of_precede_spaces: 0,
            });

            return Some(EatOpenerResult {
                token: token.with_data(TokenData {
                    marker_count,
                    indent: first_non_whitespace_index.saturating_sub(line.start_index),
                    lines,
                }),
                next_index: line.end_index,
                saturated: true,
            });
        }

        Some(EatOpenerResult {
            token,
            next_index: line.end_index,
            saturated: false,
        })
    }

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        let Some(data) = token.data_as::<TokenData>().cloned() else {
            return EatContinuationTextResult::NotMatched;
        };

        let node_points = line.node_points.as_ref();
        if line.count_of_precede_spaces < 4 && line.first_non_whitespace_index < line.end_index {
            let mut i = line.first_non_whitespace_index;
            while i < line.end_index
                && node_points[i].code_point == AsciiCodePoint::DOLLAR_SIGN as i32
            {
                i += 1;
            }

            let marker_count = i - line.first_non_whitespace_index;
            if marker_count >= data.marker_count {
                while i < line.end_index && is_space_character(node_points[i].code_point) {
                    i += 1;
                }

                if i + 1 >= line.end_index {
                    return EatContinuationTextResult::Closing {
                        next_index: line.end_index,
                    };
                }
            }
        }

        let first_index = std::cmp::min(
            line.start_index + data.indent,
            std::cmp::min(
                line.first_non_whitespace_index,
                line.end_index.saturating_sub(1),
            ),
        );
        let mut lines = data.lines;
        lines.push(PhrasingContentLine {
            node_points: line.node_points.clone(),
            start_index: first_index,
            end_index: line.end_index,
            first_non_whitespace_index: line.first_non_whitespace_index,
            count_of_precede_spaces: line.count_of_precede_spaces,
        });

        token.data = std::sync::Arc::new(TokenData {
            marker_count: data.marker_count,
            indent: data.indent,
            lines,
        });
        update_token_end_position(token, line);

        EatContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

struct MathParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for MathParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let contents = merge_content_lines_faithfully(&data.lines);
            let mut value = calc_string_from_node_points(&contents, 0, contents.len(), false);
            if !value.ends_with('\n') {
                value.push('\n');
            }

            nodes.push(Node::Math(Math {
                position: if self.api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                value,
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for MathTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(MathMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(MathParseHook { api })
    }
}

fn merge_content_lines_faithfully(lines: &[PhrasingContentLine]) -> Vec<NodePoint> {
    let mut contents = Vec::new();
    for line in lines {
        if line.start_index >= line.end_index {
            continue;
        }
        contents.extend_from_slice(&line.node_points[line.start_index..line.end_index]);
    }
    contents
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

fn update_token_end_position(token: &mut BlockToken, line: &PhrasingContentLine) {
    let Some(position) = token.position.as_mut() else {
        return;
    };
    if line.start_index >= line.end_index {
        return;
    }

    let end = line.node_points[line.end_index - 1];
    position.end = Point {
        line: end.line,
        column: end.column + 1,
        offset: Some(end.offset + 1),
    };
}
