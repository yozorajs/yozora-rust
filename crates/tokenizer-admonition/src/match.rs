use yozora_ast::{Point, Position, ADMONITION_TYPE};
use yozora_character::{
    calc_escaped_string_from_node_points, calc_trim_boundary_of_code_points, is_space_character,
    is_whitespace_character, AsciiCodePoint,
};
use yozora_core_tokenizer::{
    BlockToken, EatContinuationTextResult, EatOpenerResult, MatchBlockPhaseApi, PhrasingContentLine,
};

use crate::parse::AdmonitionTokenData;

#[derive(Debug, Clone)]
struct AdmonitionOpener {
    keyword: String,
    marker_count: usize,
    indent: usize,
    title_start: usize,
    title_end: usize,
}

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    let opener = parse_admonition_opener(line)?;

    let title_line = if opener.title_start < opener.title_end {
        Some(PhrasingContentLine {
            node_points: line.node_points.clone(),
            start_index: opener.title_start,
            end_index: opener.title_end,
            first_non_whitespace_index: opener.title_start,
            count_of_precede_spaces: 0,
        })
    } else {
        None
    };

    let token = BlockToken::new("", ADMONITION_TYPE, calc_line_position(line)).with_data(
        AdmonitionTokenData {
            keyword: opener.keyword,
            marker_count: opener.marker_count,
            indent: opener.indent,
            title_line,
            body_lines: Vec::new(),
        },
    );

    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: false,
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
    api: &dyn MatchBlockPhaseApi,
) -> EatContinuationTextResult {
    let Some(data) = token.data_as::<AdmonitionTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    if is_closing_fence(line, data.marker_count) {
        return EatContinuationTextResult::Closing {
            next_index: line.end_index,
        };
    }

    let mut body_lines = data.body_lines;
    body_lines.push(calc_content_line(line, data.indent));

    token.children = api.rollback_phrasing_lines(&body_lines, None);

    token.data = std::sync::Arc::new(AdmonitionTokenData {
        keyword: data.keyword,
        marker_count: data.marker_count,
        indent: data.indent,
        title_line: data.title_line,
        body_lines,
    });
    update_token_end_position(token, line);

    EatContinuationTextResult::Opening {
        next_index: line.end_index,
    }
}

fn parse_admonition_opener(line: &PhrasingContentLine) -> Option<AdmonitionOpener> {
    if line.count_of_precede_spaces >= 4 {
        return None;
    }

    let first_non_whitespace_index = line.first_non_whitespace_index;
    let end_index = line.end_index;
    if first_non_whitespace_index + 2 >= end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    if node_points[first_non_whitespace_index].code_point != AsciiCodePoint::COLON as i32 {
        return None;
    }

    let mut i = first_non_whitespace_index + 1;
    while i < end_index && node_points[i].code_point == AsciiCodePoint::COLON as i32 {
        i += 1;
    }

    let marker_count = i - first_non_whitespace_index;
    if marker_count < 3 {
        return None;
    }

    let (mut left_index, right_index) =
        calc_trim_boundary_of_code_points(node_points, i, end_index);
    if left_index >= right_index {
        return None;
    }

    let keyword_start = left_index;
    while left_index < right_index && !is_whitespace_character(node_points[left_index].code_point) {
        left_index += 1;
    }
    if keyword_start >= left_index {
        return None;
    }

    let keyword =
        calc_escaped_string_from_node_points(node_points, keyword_start, left_index, true);

    let mut title_start = left_index;
    while title_start < right_index && is_whitespace_character(node_points[title_start].code_point)
    {
        title_start += 1;
    }

    Some(AdmonitionOpener {
        keyword,
        marker_count,
        indent: first_non_whitespace_index.saturating_sub(line.start_index),
        title_start,
        title_end: right_index,
    })
}

fn is_closing_fence(line: &PhrasingContentLine, marker_count: usize) -> bool {
    if line.count_of_precede_spaces >= 4 {
        return false;
    }

    let first_non_whitespace_index = line.first_non_whitespace_index;
    let end_index = line.end_index;
    if first_non_whitespace_index >= end_index {
        return false;
    }

    let node_points = line.node_points.as_ref();
    let mut i = first_non_whitespace_index;
    while i < end_index && node_points[i].code_point == AsciiCodePoint::COLON as i32 {
        i += 1;
    }

    if i - first_non_whitespace_index < marker_count {
        return false;
    }

    while i < end_index {
        let code_point = node_points[i].code_point;
        if !is_space_character(code_point) {
            break;
        }
        i += 1;
    }

    i + 1 >= end_index
}

fn calc_content_line(line: &PhrasingContentLine, indent: usize) -> PhrasingContentLine {
    let first_index = std::cmp::min(
        line.start_index + indent,
        std::cmp::min(
            line.first_non_whitespace_index,
            line.end_index.saturating_sub(1),
        ),
    );

    PhrasingContentLine {
        node_points: line.node_points.clone(),
        start_index: first_index,
        end_index: line.end_index,
        first_non_whitespace_index: line.first_non_whitespace_index,
        count_of_precede_spaces: line.count_of_precede_spaces,
    }
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
