use yozora_ast::{
    AlignType, Node, Point, Position, Table, TableCell, TableColumn, TableRow, TABLE_TYPE,
};
use yozora_character::{is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::engine::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatLazyContinuationTextResult,
    EatOpenerResult, EngineBlockTokenizer, EngineTokenizer, MatchBlockHook,
    MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, RemainingSibling,
    TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const TABLE_TOKENIZER_NAME: &str = "@yozora/tokenizer-table";

#[derive(Debug, Clone)]
pub struct TableTokenizer {
    meta: TokenizerMeta,
}

impl Default for TableTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: TABLE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 5,
            },
        }
    }
}

impl Tokenizer for TableTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for TableTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_table_token(lines)?;
        Some(parse::parse_table_token(token))
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
struct TableCellTokenData {
    position: Option<Position>,
    lines: Vec<PhrasingContentLine>,
}

#[derive(Debug, Clone)]
struct TableRowTokenData {
    position: Option<Position>,
    cells: Vec<TableCellTokenData>,
}

#[derive(Debug, Clone)]
struct TokenData {
    columns: Vec<TableColumn>,
    rows: Vec<TableRowTokenData>,
}

impl EngineTokenizer for TableTokenizer {
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

struct TableMatchHook<'a> {
    api: &'a dyn EngineMatchBlockPhaseApi,
}

impl MatchBlockHook for TableMatchHook<'_> {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        _line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        None
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        if line.count_of_precede_spaces >= 4 || line.first_non_whitespace_index >= line.end_index {
            return None;
        }

        let columns = calc_delimiter_columns(line)?;
        if columns.is_empty() {
            return None;
        }

        let lines = self.api.extract_phrasing_lines(prev_sibling_token)?;
        if lines.is_empty() {
            return None;
        }

        let previous_line = lines.last()?;
        if !is_header_cell_count_matched(previous_line, columns.len()) {
            return None;
        }

        let row = calc_table_row(previous_line, &columns)?;
        let position = calc_spanning_position(previous_line, line);

        let token = BlockToken::new("", TABLE_TYPE, position).with_data(TokenData {
            columns,
            rows: vec![row],
        });

        let remaining_lines = &lines[..lines.len() - 1];
        let remaining_sibling_tokens = self
            .api
            .rollback_phrasing_lines(remaining_lines, Some(prev_sibling_token));

        let remaining_sibling = match remaining_sibling_tokens.len() {
            0 => RemainingSibling::None,
            1 => RemainingSibling::One(remaining_sibling_tokens[0].clone()),
            _ => RemainingSibling::Many(remaining_sibling_tokens),
        };

        Some(EatAndInterruptPreviousSiblingResult {
            token,
            next_index: line.end_index,
            saturated: false,
            remaining_sibling,
        })
    }

    fn eat_lazy_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatLazyContinuationTextResult {
        if line.first_non_whitespace_index >= line.end_index {
            return EatLazyContinuationTextResult::NotMatched;
        }

        let Some(data) = token.data_as::<TokenData>().cloned() else {
            return EatLazyContinuationTextResult::NotMatched;
        };

        let Some(row) = calc_table_row(line, &data.columns) else {
            return EatLazyContinuationTextResult::NotMatched;
        };

        let mut rows = data.rows;
        rows.push(row);

        token.data = std::sync::Arc::new(TokenData {
            columns: data.columns,
            rows,
        });
        update_token_end_position(token, line);

        EatLazyContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

struct TableParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for TableParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            let mut rows = Vec::with_capacity(data.rows.len());
            for row in &data.rows {
                let mut cells = Vec::with_capacity(row.cells.len());
                for cell in &row.cells {
                    let merged = merge_and_strip_content_lines(&cell.lines);
                    let contents = unescape_table_cell_contents(&merged);
                    let children = self.api.process_inlines(&contents);

                    cells.push(Node::TableCell(TableCell {
                        position: if self.api.should_reserve_position() {
                            cell.position.clone()
                        } else {
                            None
                        },
                        children,
                    }));
                }

                rows.push(Node::TableRow(TableRow {
                    position: if self.api.should_reserve_position() {
                        row.position.clone()
                    } else {
                        None
                    },
                    children: cells,
                }));
            }

            nodes.push(Node::Table(Table {
                position: if self.api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                columns: data.columns.clone(),
                children: rows,
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for TableTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(TableMatchHook { api })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(TableParseHook { api })
    }
}

fn calc_delimiter_columns(line: &PhrasingContentLine) -> Option<Vec<TableColumn>> {
    let node_points = line.node_points.as_ref();
    let end_index = line.end_index;
    let mut c_index = line.first_non_whitespace_index;

    if c_index < end_index
        && node_points[c_index].code_point == AsciiCodePoint::VERTICAL_SLASH as i32
    {
        c_index += 1;
    }

    let mut columns = Vec::new();
    while c_index < end_index {
        while c_index < end_index && is_whitespace_character(node_points[c_index].code_point) {
            c_index += 1;
        }
        if c_index >= end_index {
            break;
        }

        let mut left_colon = false;
        if node_points[c_index].code_point == AsciiCodePoint::COLON as i32 {
            left_colon = true;
            c_index += 1;
        }

        let mut hyphen_count = 0usize;
        while c_index < end_index
            && node_points[c_index].code_point == AsciiCodePoint::MINUS_SIGN as i32
        {
            hyphen_count += 1;
            c_index += 1;
        }
        if hyphen_count == 0 {
            return None;
        }

        let mut right_colon = false;
        if c_index < end_index && node_points[c_index].code_point == AsciiCodePoint::COLON as i32 {
            right_colon = true;
            c_index += 1;
        }

        while c_index < end_index {
            let code_point = node_points[c_index].code_point;
            if is_whitespace_character(code_point) {
                c_index += 1;
                continue;
            }

            if code_point == AsciiCodePoint::VERTICAL_SLASH as i32 {
                c_index += 1;
                break;
            }

            return None;
        }

        let align = if left_colon && right_colon {
            Some(AlignType::Center)
        } else if left_colon {
            Some(AlignType::Left)
        } else if right_colon {
            Some(AlignType::Right)
        } else {
            None
        };

        columns.push(TableColumn { align });
    }

    if columns.is_empty() {
        None
    } else {
        Some(columns)
    }
}

fn is_header_cell_count_matched(line: &PhrasingContentLine, expected_columns: usize) -> bool {
    let node_points = line.node_points.as_ref();
    let mut cell_count = 0usize;
    let mut has_non_whitespace_before_pipe = false;

    let mut index = line.start_index;
    while index < line.end_index {
        let code_point = node_points[index].code_point;
        if is_whitespace_character(code_point) {
            index += 1;
            continue;
        }

        if code_point == AsciiCodePoint::VERTICAL_SLASH as i32 {
            if has_non_whitespace_before_pipe || cell_count > 0 {
                cell_count += 1;
            }
            has_non_whitespace_before_pipe = false;
            index += 1;
            continue;
        }

        has_non_whitespace_before_pipe = true;
        if code_point == AsciiCodePoint::BACKSLASH as i32 {
            index += 1;
        }
        index += 1;
    }

    if has_non_whitespace_before_pipe && expected_columns > 1 {
        cell_count += 1;
    }

    cell_count == expected_columns
}

fn calc_table_row(
    line: &PhrasingContentLine,
    columns: &[TableColumn],
) -> Option<TableRowTokenData> {
    if line.start_index >= line.end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let start_index = line.start_index;
    let end_index = line.end_index;
    let mut i = line.first_non_whitespace_index;

    if i < end_index && node_points[i].code_point == AsciiCodePoint::VERTICAL_SLASH as i32 {
        i += 1;
    }

    let mut cells = Vec::new();
    while i < end_index {
        while i < end_index && is_whitespace_character(node_points[i].code_point) {
            i += 1;
        }

        let start_point = if i < end_index {
            calc_start_point(node_points, i)
        } else {
            calc_end_point(node_points, end_index - 1)
        };

        let cell_start_index = i;
        let cell_first_non_whitespace_index = i;
        while i < end_index {
            let code_point = node_points[i].code_point;
            if code_point == AsciiCodePoint::BACKSLASH as i32 {
                i = std::cmp::min(i + 2, end_index);
                continue;
            }

            if code_point == AsciiCodePoint::VERTICAL_SLASH as i32 {
                break;
            }

            i += 1;
        }

        let mut cell_end_index = i;
        while cell_end_index > cell_start_index
            && is_whitespace_character(node_points[cell_end_index - 1].code_point)
        {
            cell_end_index -= 1;
        }

        let end_point = if i > 0 {
            calc_end_point(node_points, i - 1)
        } else {
            start_point
        };

        let lines = if cell_first_non_whitespace_index >= cell_end_index {
            Vec::new()
        } else {
            vec![PhrasingContentLine {
                node_points: line.node_points.clone(),
                start_index: cell_start_index,
                end_index: cell_end_index,
                first_non_whitespace_index: cell_first_non_whitespace_index,
                count_of_precede_spaces: cell_first_non_whitespace_index
                    .saturating_sub(cell_start_index),
            }]
        };

        cells.push(TableCellTokenData {
            position: Some(Position {
                start: start_point,
                end: end_point,
                indent: None,
            }),
            lines,
        });

        if cells.len() >= columns.len() {
            break;
        }

        if i < end_index && node_points[i].code_point == AsciiCodePoint::VERTICAL_SLASH as i32 {
            i += 1;
        }
    }

    let row_start = calc_start_point(node_points, start_index);
    let row_end = calc_end_point(node_points, end_index - 1);
    while cells.len() < columns.len() {
        cells.push(TableCellTokenData {
            position: Some(Position {
                start: row_end,
                end: row_end,
                indent: None,
            }),
            lines: Vec::new(),
        });
    }

    Some(TableRowTokenData {
        position: Some(Position {
            start: row_start,
            end: row_end,
            indent: None,
        }),
        cells,
    })
}

fn merge_and_strip_content_lines(lines: &[PhrasingContentLine]) -> Vec<NodePoint> {
    let mut merged = Vec::new();
    for line in lines {
        if line.start_index >= line.end_index {
            continue;
        }
        merged.extend_from_slice(&line.node_points[line.start_index..line.end_index]);
    }
    merged
}

fn unescape_table_cell_contents(node_points: &[NodePoint]) -> Vec<NodePoint> {
    let mut contents = Vec::new();
    let mut i = 0usize;
    while i < node_points.len() {
        let point = node_points[i];
        if point.code_point == AsciiCodePoint::BACKSLASH as i32 && i + 1 < node_points.len() {
            let next = node_points[i + 1];
            if next.code_point != AsciiCodePoint::VERTICAL_SLASH as i32 {
                contents.push(point);
            }
            contents.push(next);
            i += 2;
            continue;
        }

        contents.push(point);
        i += 1;
    }

    contents
}

fn calc_start_point(node_points: &[NodePoint], index: usize) -> Point {
    let point = node_points[index];
    Point {
        line: point.line,
        column: point.column,
        offset: Some(point.offset),
    }
}

fn calc_end_point(node_points: &[NodePoint], index: usize) -> Point {
    let point = node_points[index];
    Point {
        line: point.line,
        column: point.column + 1,
        offset: Some(point.offset + 1),
    }
}

fn calc_spanning_position(
    first: &PhrasingContentLine,
    last: &PhrasingContentLine,
) -> Option<Position> {
    if first.start_index >= first.end_index || last.start_index >= last.end_index {
        return None;
    }

    Some(Position {
        start: calc_start_point(first.node_points.as_ref(), first.start_index),
        end: calc_end_point(last.node_points.as_ref(), last.end_index - 1),
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

    position.end = calc_end_point(line.node_points.as_ref(), line.end_index - 1);
}
