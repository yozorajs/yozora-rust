use yozora_ast::{Definition, Node, Point, Position, DEFINITION_TYPE};
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

pub const DEFINITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-definition";

#[derive(Debug, Clone)]
pub struct DefinitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for DefinitionTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: DEFINITION_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 8,
            },
        }
    }
}

impl Tokenizer for DefinitionTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for DefinitionTokenizer {
    fn can_interrupt_paragraph(&self) -> bool {
        false
    }

    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_definition(lines)?;
        Some(parse::parse_definition_token(token))
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
        api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_definition(lines)?;
        api.register_definition_identifier(&token.identifier);
        Some(parse::parse_definition_token(token))
    }
}

#[derive(Debug, Clone)]
struct TokenData {
    lines: Vec<String>,
    latest: r#match::DefinitionBlockToken,
}

impl EngineTokenizer for DefinitionTokenizer {
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

struct DefinitionMatchHook<'a> {
    api: &'a dyn EngineMatchBlockPhaseApi,
}

impl DefinitionMatchHook<'_> {
    fn register_identifier(&self, token: &r#match::DefinitionBlockToken) {
        self.api.register_definition_identifier(&token.identifier);
    }
}

impl MatchBlockHook for DefinitionMatchHook<'_> {
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
        let matched = r#match::match_definition(&refs)?;
        self.register_identifier(&matched);

        let token =
            BlockToken::new("", DEFINITION_TYPE, calc_line_position(line)).with_data(TokenData {
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
        let Some(matched) = r#match::match_definition(&refs) else {
            return EatContinuationTextResult::NotMatched;
        };

        if matched.consumed_lines < lines.len() {
            return EatContinuationTextResult::NotMatched;
        }

        self.register_identifier(&matched);
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

struct DefinitionParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for DefinitionParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let mut node = parse::parse_definition_token(data.latest.clone()).node;
            if let Node::Definition(Definition { position, .. }) = &mut node {
                if self.api.should_reserve_position() {
                    *position = token.position.clone();
                }
            }
            nodes.push(node);
        }

        nodes
    }
}

impl EngineBlockTokenizer for DefinitionTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(DefinitionMatchHook { api })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(DefinitionParseHook { api })
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
