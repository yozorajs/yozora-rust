use yozora_ast::{Html, Node, Point, Position, HTML_TYPE};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::engine::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    EngineBlockTokenizer, EngineTokenizer, MatchBlockHook,
    MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, RemainingSibling,
    TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const HTML_BLOCK_TOKENIZER_NAME: &str = "@yozora/tokenizer-html-block";

#[derive(Debug, Clone)]
pub struct HtmlBlockTokenizer {
    meta: TokenizerMeta,
}

impl Default for HtmlBlockTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: HTML_BLOCK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 2,
            },
        }
    }
}

impl Tokenizer for HtmlBlockTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for HtmlBlockTokenizer {
    fn can_interrupt_paragraph_with_lines(&self, lines: &[&str]) -> bool {
        r#match::can_interrupt_paragraph_with_lines(lines)
    }

    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_html_block_token(lines)?;
        Some(parse::parse_html_block_token(token))
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
    kind: r#match::HtmlBlockKind,
    lines: Vec<PhrasingContentLine>,
}

impl EngineTokenizer for HtmlBlockTokenizer {
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

struct HtmlBlockMatchHook;

impl MatchBlockHook for HtmlBlockMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        let source = calc_line_text(line);
        let kind = r#match::detect_html_block_kind(&source)?;

        let token = BlockToken::new("", HTML_TYPE, calc_line_position(line)).with_data(TokenData {
            kind: kind.clone(),
            lines: vec![line.clone()],
        });

        let saturated = !kind.ends_on_blank_line() && kind.is_closed_by_line(&source);
        Some(EatOpenerResult {
            token,
            next_index: line.end_index,
            saturated,
        })
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        let opener = self.eat_opener(line, prev_sibling_token)?;
        let data = opener.token.data_as::<TokenData>()?;
        if data.kind == r#match::HtmlBlockKind::Type7 {
            return None;
        }

        Some(EatAndInterruptPreviousSiblingResult {
            token: opener.token,
            next_index: opener.next_index,
            saturated: opener.saturated,
            remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
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

        let source = calc_line_text(line);
        if data.kind.ends_on_blank_line() && source.trim().is_empty() {
            return EatContinuationTextResult::NotMatched;
        }

        let mut lines = data.lines;
        lines.push(line.clone());
        let should_close = !data.kind.ends_on_blank_line() && data.kind.is_closed_by_line(&source);

        token.data = std::sync::Arc::new(TokenData {
            kind: data.kind,
            lines,
        });
        update_token_end_position(token, line);

        if should_close {
            EatContinuationTextResult::Closing {
                next_index: line.end_index,
            }
        } else {
            EatContinuationTextResult::Opening {
                next_index: line.end_index,
            }
        }
    }
}

struct HtmlBlockParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for HtmlBlockParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let contents = merge_content_lines_faithfully(&data.lines);
            let value = calc_string_from_node_points(&contents, 0, contents.len(), false);
            nodes.push(Node::Html(Html {
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

impl EngineBlockTokenizer for HtmlBlockTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(HtmlBlockMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(HtmlBlockParseHook { api })
    }
}

fn merge_content_lines_faithfully(
    lines: &[PhrasingContentLine],
) -> Vec<yozora_character::NodePoint> {
    let mut contents = Vec::new();
    for line in lines {
        if line.start_index >= line.end_index {
            continue;
        }

        contents.extend_from_slice(&line.node_points[line.start_index..line.end_index]);
    }
    contents
}

fn calc_line_text(line: &PhrasingContentLine) -> String {
    calc_string_from_node_points(&line.node_points, line.start_index, line.end_index, false)
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
