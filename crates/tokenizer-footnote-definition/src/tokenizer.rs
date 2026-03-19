use yozora_ast::{FootnoteDefinition, Node, Point, Position, FOOTNOTE_DEFINITION_TYPE};
use yozora_character::{calc_string_from_node_points, is_whitespace_character, AsciiCodePoint};
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

pub const FOOTNOTE_DEFINITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-definition";

#[derive(Debug, Clone)]
pub struct FootnoteDefinitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteDefinitionTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FOOTNOTE_DEFINITION_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 9,
            },
        }
    }
}

impl Tokenizer for FootnoteDefinitionTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for FootnoteDefinitionTokenizer {
    fn can_interrupt_paragraph(&self) -> bool {
        false
    }

    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_footnote_definition(lines)?;
        Some(parse::parse_footnote_definition_token(token))
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
        api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_footnote_definition(lines)?;
        api.register_footnote_definition_identifier(&token.identifier);
        Some(parse::parse_footnote_definition_token(token))
    }
}

#[derive(Debug, Clone)]
struct TokenData {
    label: String,
    identifier: String,
}

impl EngineTokenizer for FootnoteDefinitionTokenizer {
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

struct FootnoteDefinitionMatchHook<'a> {
    api: &'a dyn EngineMatchBlockPhaseApi,
}

impl MatchBlockHook for FootnoteDefinitionMatchHook<'_> {
    fn is_containing_block(&self) -> bool {
        true
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        if line.count_of_precede_spaces >= 4 {
            return None;
        }

        let node_points = line.node_points.as_ref();
        let first_non_whitespace_index = line.first_non_whitespace_index;
        let Some(label_end) =
            eat_footnote_label(node_points, first_non_whitespace_index, line.end_index)
        else {
            return None;
        };

        if label_end + 1 >= line.end_index
            || node_points[label_end + 1].code_point != AsciiCodePoint::COLON as i32
        {
            return None;
        }

        let label = calc_string_from_node_points(
            node_points,
            first_non_whitespace_index + 2,
            label_end,
            false,
        )
        .trim()
        .to_string();
        if label.is_empty() {
            return None;
        }

        let identifier = normalize_identifier(&label);
        self.api
            .register_footnote_definition_identifier(&identifier);

        let token = BlockToken::new("", FOOTNOTE_DEFINITION_TYPE, calc_line_position(line))
            .with_data(TokenData { label, identifier });

        Some(EatOpenerResult {
            token,
            next_index: label_end + 2,
            saturated: false,
        })
    }

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        update_token_end_position(token, line);

        const INDENT: usize = 4;
        if line.first_non_whitespace_index >= line.end_index {
            return EatContinuationTextResult::Opening {
                next_index: std::cmp::min(
                    line.end_index.saturating_sub(1),
                    line.start_index + INDENT,
                ),
            };
        }

        if line.count_of_precede_spaces >= INDENT {
            return EatContinuationTextResult::Opening {
                next_index: line.start_index + INDENT,
            };
        }

        EatContinuationTextResult::NotMatched
    }
}

struct FootnoteDefinitionParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for FootnoteDefinitionParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let children = self.api.parse_block_tokens(&token.children);
            nodes.push(Node::FootnoteDefinition(FootnoteDefinition {
                position: if self.api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                identifier: data.identifier.clone(),
                label: data.label.clone(),
                children,
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for FootnoteDefinitionTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(FootnoteDefinitionMatchHook { api })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(FootnoteDefinitionParseHook { api })
    }
}

fn eat_footnote_label(
    node_points: &[yozora_character::NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<usize> {
    if start_index + 2 >= end_index {
        return None;
    }

    if node_points[start_index].code_point != AsciiCodePoint::OPEN_BRACKET as i32
        || node_points[start_index + 1].code_point != AsciiCodePoint::CARET as i32
    {
        return None;
    }

    let mut i = start_index + 2;
    let mut has_non_whitespace = false;
    while i < end_index {
        let code_point = node_points[i].code_point;
        if code_point == AsciiCodePoint::BACKSLASH as i32 {
            if i + 1 >= end_index {
                return None;
            }

            if !is_whitespace_character(node_points[i + 1].code_point) {
                has_non_whitespace = true;
            }
            i += 2;
            continue;
        }

        if code_point == AsciiCodePoint::OPEN_BRACKET as i32 {
            return None;
        }

        if code_point == AsciiCodePoint::CLOSE_BRACKET as i32 {
            if !has_non_whitespace {
                return None;
            }
            return Some(i);
        }

        if !is_whitespace_character(code_point) {
            has_non_whitespace = true;
        }
        i += 1;
    }

    None
}

fn normalize_identifier(label: &str) -> String {
    let mut out = String::new();
    for (idx, chunk) in label.split_whitespace().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(chunk);
    }
    out.to_ascii_lowercase()
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
