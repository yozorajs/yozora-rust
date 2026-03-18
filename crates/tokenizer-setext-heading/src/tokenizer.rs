use yozora_ast::{Heading, Node, Point, Position, HEADING_TYPE};
use yozora_character::VirtualCodePoint;
use yozora_core_tokenizer::engine::{
    BlockToken, EatContinuationTextResult, EatOpenerResult, EngineBlockTokenizer, EngineTokenizer,
    MatchBlockHook, MatchBlockPhaseApi as EngineMatchBlockPhaseApi, OnCloseResult, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, TokenizerType,
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
                priority: 5,
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
    lines: Vec<String>,
    raw_lines: Vec<PhrasingContentLine>,
    latest: Option<r#match::SetextHeadingToken>,
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

struct SetextHeadingMatchHook;

impl MatchBlockHook for SetextHeadingMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        let source = line_to_string(line);
        if !looks_like_setext_start(&source) {
            return None;
        }

        let token =
            BlockToken::new("", HEADING_TYPE, calc_line_position(line)).with_data(TokenData {
                lines: vec![source],
                raw_lines: vec![line.clone()],
                latest: None,
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
        let Some(data) = token.data_as::<TokenData>() else {
            return EatContinuationTextResult::NotMatched;
        };

        let mut lines = data.lines.clone();
        let mut raw_lines = data.raw_lines.clone();
        let current_line = line_to_string(line);
        lines.push(current_line.clone());
        raw_lines.push(line.clone());

        let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
        let matched = r#match::match_setext_heading_token(&refs);

        if let Some(matched) = &matched {
            if matched.consumed_lines < lines.len() {
                return EatContinuationTextResult::NotMatched;
            }
        } else if data.latest.is_some() {
            return EatContinuationTextResult::NotMatched;
        } else if !looks_like_setext_continuation(&current_line) {
            return EatContinuationTextResult::FailedAndRollback { lines: raw_lines };
        }

        token.data = std::sync::Arc::new(TokenData {
            lines,
            raw_lines,
            latest: matched,
        });
        update_token_end_position(token, line);

        EatContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }

    fn on_close(&mut self, token: &BlockToken) -> Option<OnCloseResult> {
        let data = token.data_as::<TokenData>()?;
        if data.latest.is_some() {
            return None;
        }

        Some(OnCloseResult::FailedAndRollback {
            lines: data.raw_lines.clone(),
        })
    }
}

struct SetextHeadingParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for SetextHeadingParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::new();

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };
            let Some(matched) = data.latest.clone() else {
                continue;
            };

            let mut node = parse::parse_setext_heading_token(matched).node;
            if let Node::Heading(Heading { position, .. }) = &mut node {
                if self.api.should_reserve_position() {
                    *position = token.position.clone();
                }
            }
            nodes.push(node);
        }

        nodes
    }
}

impl EngineBlockTokenizer for SetextHeadingTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(SetextHeadingMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(SetextHeadingParseHook { api })
    }
}

fn looks_like_setext_start(line: &str) -> bool {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return false;
    }

    !line.trim().is_empty()
}

fn looks_like_setext_continuation(line: &str) -> bool {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return false;
    }

    !line.trim().is_empty()
}

fn line_to_string(line: &PhrasingContentLine) -> String {
    let mut source = String::new();
    for point in line
        .node_points
        .iter()
        .skip(line.start_index)
        .take(line.end_index.saturating_sub(line.start_index))
    {
        let code_point = point.code_point;
        let ch = if code_point == VirtualCodePoint::Space as i32 {
            Some(' ')
        } else if code_point == VirtualCodePoint::LineEnd as i32 {
            Some('\n')
        } else {
            char::from_u32(code_point as u32)
        };

        if let Some(ch) = ch {
            source.push(ch);
        }
    }
    source
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
