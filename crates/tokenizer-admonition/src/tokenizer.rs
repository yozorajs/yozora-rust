use yozora_ast::{Admonition, Node, Point, Position, ADMONITION_TYPE};
use yozora_character::VirtualCodePoint;
use yozora_core_tokenizer::engine::{
    BlockToken, EatContinuationTextResult, EatOpenerResult, EngineBlockTokenizer, EngineTokenizer,
    MatchBlockHook, MatchBlockPhaseApi as EngineMatchBlockPhaseApi, OnCloseResult, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

use crate::{parse, r#match};

pub const ADMONITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-admonition";

#[derive(Debug, Clone)]
pub struct AdmonitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for AdmonitionTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: ADMONITION_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 12,
            },
        }
    }
}

impl Tokenizer for AdmonitionTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for AdmonitionTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_admonition(lines)?;
        Some(parse::parse_admonition_block(lines, position, token))
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
        _api: &mut dyn yozora_core_tokenizer::MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        self.tokenize_block_lines(lines, position)
    }
}

#[derive(Debug, Clone)]
struct TokenData {
    lines: Vec<String>,
    raw_lines: Vec<PhrasingContentLine>,
    latest: Option<r#match::AdmonitionBlockToken>,
}

impl EngineTokenizer for AdmonitionTokenizer {
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

struct AdmonitionMatchHook;

impl MatchBlockHook for AdmonitionMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        let source = line_to_string(line);
        if !looks_like_admonition_opener(&source) {
            return None;
        }

        let refs = [source.as_str()];
        let latest = r#match::match_admonition(&refs);

        let token =
            BlockToken::new("", ADMONITION_TYPE, calc_line_position(line)).with_data(TokenData {
                lines: vec![source],
                raw_lines: vec![line.clone()],
                latest,
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
        lines.push(line_to_string(line));
        raw_lines.push(line.clone());

        let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
        let matched = r#match::match_admonition(&refs);

        if let Some(matched) = &matched {
            if matched.consumed_lines < lines.len() {
                return EatContinuationTextResult::NotMatched;
            }
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

struct AdmonitionParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for AdmonitionParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::new();

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };
            let Some(matched) = data.latest.clone() else {
                continue;
            };

            let mut node = parse::parse_admonition_block(&[], token.position.clone(), matched).node;
            if let Node::Admonition(Admonition { position, .. }) = &mut node {
                if self.api.should_reserve_position() {
                    *position = token.position.clone();
                }
            }
            nodes.push(node);
        }

        nodes
    }
}

impl EngineBlockTokenizer for AdmonitionTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(AdmonitionMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(AdmonitionParseHook { api })
    }
}

fn looks_like_admonition_opener(line: &str) -> bool {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return false;
    }

    line[leading_spaces..].trim_end().starts_with(":::")
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
