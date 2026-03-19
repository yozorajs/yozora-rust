use yozora_ast::{AlignType, Position, TableColumn, TABLE_TYPE};
use yozora_character::{is_whitespace_character, AsciiCodePoint};
use yozora_core_tokenizer::*;

#[derive(Debug, Clone)]
pub(crate) struct TableCellTokenData {
    pub position: Option<Position>,
    pub lines: Vec<PhrasingContentLine>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableRowTokenData {
    pub position: Option<Position>,
    pub cells: Vec<TableCellTokenData>,
}

#[derive(Debug, Clone)]
pub(crate) struct TokenData {
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRowTokenData>,
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
    match_api: &dyn MatchBlockPhaseApi,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    if line.count_of_precede_spaces >= 4 || line.first_non_whitespace_index >= line.end_index {
        return None;
    }

    let columns = calc_delimiter_columns(line)?;
    if columns.is_empty() {
        return None;
    }

    let lines = match_api.extract_phrasing_lines(prev_sibling_token)?;
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
    let remaining_sibling_tokens =
        match_api.rollback_phrasing_lines(remaining_lines, Some(prev_sibling_token));

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

pub(crate) fn eat_lazy_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
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
