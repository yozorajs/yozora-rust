use yozora_ast::{Code, Node, Point, Position, CODE_TYPE};
use yozora_character::{
    calc_escaped_string_from_node_points, calc_string_from_node_points,
    calc_trim_boundary_of_code_points, is_space_character, is_whitespace_character, AsciiCodePoint,
    NodePoint,
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

pub const FENCED_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-fenced-code";

#[derive(Debug, Clone)]
pub struct FencedCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for FencedCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FENCED_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 10,
            },
        }
    }
}

impl Tokenizer for FencedCodeTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for FencedCodeTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_fenced_code_token(lines)?;
        Some(parse::parse_fenced_code_token(token))
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
    marker_count: usize,
    indent: usize,
    info_string: Vec<NodePoint>,
    lines: Vec<PhrasingContentLine>,
}

impl EngineTokenizer for FencedCodeTokenizer {
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

struct FencedCodeMatchHook;

impl MatchBlockHook for FencedCodeMatchHook {
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
        if first_non_whitespace_index + 2 >= line.end_index {
            return None;
        }

        let node_points = line.node_points.as_ref();
        let marker = node_points[first_non_whitespace_index].code_point;
        if marker != AsciiCodePoint::BACKTICK as i32 && marker != AsciiCodePoint::TILDE as i32 {
            return None;
        }

        let mut i = first_non_whitespace_index + 1;
        while i < line.end_index && node_points[i].code_point == marker {
            i += 1;
        }

        let marker_count = i - first_non_whitespace_index;
        if marker_count < 3 {
            return None;
        }

        let (left, right) = calc_trim_boundary_of_code_points(node_points, i, line.end_index);
        let info_string = node_points[left..right].to_vec();
        if marker == AsciiCodePoint::BACKTICK as i32
            && info_string
                .iter()
                .any(|point| point.code_point == AsciiCodePoint::BACKTICK as i32)
        {
            return None;
        }

        let token = BlockToken::new("", CODE_TYPE, calc_line_position(line)).with_data(TokenData {
            marker,
            marker_count,
            indent: first_non_whitespace_index.saturating_sub(line.start_index),
            info_string,
            lines: Vec::new(),
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

        let node_points = line.node_points.as_ref();
        if line.count_of_precede_spaces < 4 && line.first_non_whitespace_index < line.end_index {
            let mut i = line.first_non_whitespace_index;
            while i < line.end_index && node_points[i].code_point == data.marker {
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
            marker: data.marker,
            marker_count: data.marker_count,
            indent: data.indent,
            info_string: data.info_string,
            lines,
        });
        update_token_end_position(token, line);

        EatContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

struct FencedCodeParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for FencedCodeParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let (lang, meta) = parse_info_string(&data.info_string);
            let contents = merge_content_lines_faithfully(&data.lines);
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
                lang,
                meta,
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for FencedCodeTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(FencedCodeMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(FencedCodeParseHook { api })
    }
}

fn parse_info_string(info_string: &[NodePoint]) -> (Option<String>, Option<String>) {
    let mut i = 0usize;
    while i < info_string.len() && !is_whitespace_character(info_string[i].code_point) {
        i += 1;
    }

    let lang = calc_escaped_string_from_node_points(info_string, 0, i, true);
    while i < info_string.len() && is_whitespace_character(info_string[i].code_point) {
        i += 1;
    }
    let meta = calc_escaped_string_from_node_points(info_string, i, info_string.len(), true);

    (
        if lang.is_empty() { None } else { Some(lang) },
        if meta.is_empty() { None } else { Some(meta) },
    )
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
