use yozora_ast::{Code, Node, Point, Position, CODE_TYPE};
use yozora_character::{calc_string_from_node_points, AsciiCodePoint, NodePoint, VirtualCodePoint};
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

pub const INDENTED_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-indented-code";

#[derive(Debug, Clone)]
pub struct IndentedCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for IndentedCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: INDENTED_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 4,
            },
        }
    }
}

impl Tokenizer for IndentedCodeTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for IndentedCodeTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_indented_code_token(lines)?;
        Some(parse::parse_indented_code_token(token))
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
    lines: Vec<PhrasingContentLine>,
}

impl EngineTokenizer for IndentedCodeTokenizer {
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

struct IndentedCodeMatchHook;

impl MatchBlockHook for IndentedCodeMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        if line.count_of_precede_spaces < 4 {
            return None;
        }

        let mut first_index = line.start_index + 4;
        if line.start_index + 3 < line.node_points.len()
            && line.node_points[line.start_index].code_point == AsciiCodePoint::SPACE as i32
            && line.node_points[line.start_index + 3].code_point == VirtualCodePoint::Space as i32
        {
            let mut i = line.start_index + 1;
            while i < line.first_non_whitespace_index {
                if line.node_points[i].code_point == VirtualCodePoint::Space as i32 {
                    break;
                }
                i += 1;
            }
            first_index = i + 4;
        }

        let token = BlockToken::new("", CODE_TYPE, calc_line_position(line)).with_data(TokenData {
            lines: vec![PhrasingContentLine {
                node_points: line.node_points.clone(),
                start_index: first_index,
                end_index: line.end_index,
                first_non_whitespace_index: line.first_non_whitespace_index,
                count_of_precede_spaces: line
                    .count_of_precede_spaces
                    .saturating_sub(first_index.saturating_sub(line.start_index)),
            }],
        });

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

        if line.count_of_precede_spaces < 4 && line.first_non_whitespace_index < line.end_index {
            return EatContinuationTextResult::NotMatched;
        }

        let first_index = std::cmp::min(line.end_index.saturating_sub(1), line.start_index + 4);
        let mut lines = data.lines;
        lines.push(PhrasingContentLine {
            node_points: line.node_points.clone(),
            start_index: first_index,
            end_index: line.end_index,
            first_non_whitespace_index: line.first_non_whitespace_index,
            count_of_precede_spaces: line
                .count_of_precede_spaces
                .saturating_sub(first_index.saturating_sub(line.start_index)),
        });

        token.data = std::sync::Arc::new(TokenData { lines });
        update_token_end_position(token, line);

        EatContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

struct IndentedCodeParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for IndentedCodeParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let mut start_line_index = 0usize;
            let mut end_line_index = data.lines.len();

            while start_line_index < end_line_index {
                let line = &data.lines[start_line_index];
                if line.first_non_whitespace_index < line.end_index {
                    break;
                }
                start_line_index += 1;
            }

            while start_line_index < end_line_index {
                let line = &data.lines[end_line_index - 1];
                if line.first_non_whitespace_index < line.end_index {
                    break;
                }
                end_line_index -= 1;
            }

            let contents =
                merge_content_lines_faithfully(&data.lines, start_line_index, end_line_index);
            let mut value = calc_string_from_node_points(&contents, 0, contents.len(), false);
            if !value.ends_with('\n') {
                value.push('\n');
            }

            nodes.push(Node::Code(Code {
                position: if self.api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                value,
                lang: None,
                meta: None,
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for IndentedCodeTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(IndentedCodeMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(IndentedCodeParseHook { api })
    }
}

fn merge_content_lines_faithfully(
    lines: &[PhrasingContentLine],
    start_line_index: usize,
    end_line_index: usize,
) -> Vec<NodePoint> {
    if start_line_index >= end_line_index || end_line_index > lines.len() {
        return Vec::new();
    }

    let mut contents = Vec::new();
    for line in lines.iter().take(end_line_index).skip(start_line_index) {
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
