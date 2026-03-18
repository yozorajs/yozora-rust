use yozora_ast::{Blockquote, Node, Point, Position, BLOCKQUOTE_TYPE};
use yozora_character::VirtualCodePoint;
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

#[derive(Debug, Clone)]
struct TokenData {
    lines: Vec<String>,
    latest: r#match::BlockquoteToken,
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
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        let source = line_to_string(line);
        let refs = [source.as_str()];
        let matched = r#match::match_blockquote_token(&refs)?;

        let token =
            BlockToken::new("", BLOCKQUOTE_TYPE, calc_line_position(line)).with_data(TokenData {
                lines: vec![source],
                latest: matched,
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
        lines.push(line_to_string(line));

        let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
        let Some(matched) = r#match::match_blockquote_token(&refs) else {
            return EatContinuationTextResult::NotMatched;
        };

        if matched.consumed_lines < lines.len() {
            return EatContinuationTextResult::NotMatched;
        }

        token.data = std::sync::Arc::new(TokenData {
            lines,
            latest: matched,
        });
        update_token_end_position(token, line);

        EatContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

struct BlockquoteParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for BlockquoteParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let mut node = parse::parse_blockquote_token(data.latest.clone()).node;
            if let Node::Blockquote(Blockquote { position, .. }) = &mut node {
                if self.api.should_reserve_position() {
                    *position = token.position.clone();
                }
            }
            nodes.push(node);
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
