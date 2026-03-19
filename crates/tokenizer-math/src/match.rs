use yozora_ast::{Point, Position, MATH_TYPE};
use yozora_character::{calc_trim_boundary_of_code_points, is_space_character, AsciiCodePoint};
use yozora_core_tokenizer::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    PhrasingContentLine, RemainingSibling,
};

use crate::parse::MathTokenData;

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.count_of_precede_spaces >= 4 {
        return None;
    }

    let first_non_whitespace_index = line.first_non_whitespace_index;
    if first_non_whitespace_index + 1 >= line.end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    if node_points[first_non_whitespace_index].code_point != AsciiCodePoint::DOLLAR_SIGN as i32 {
        return None;
    }

    let mut i = first_non_whitespace_index + 1;
    while i < line.end_index && node_points[i].code_point == AsciiCodePoint::DOLLAR_SIGN as i32 {
        i += 1;
    }

    let marker_count = i - first_non_whitespace_index;
    if marker_count < 2 {
        return None;
    }

    let (left, right) = calc_trim_boundary_of_code_points(node_points, i, line.end_index);

    let token = BlockToken::new("", MATH_TYPE, calc_line_position(line)).with_data(MathTokenData {
        marker_count,
        indent: first_non_whitespace_index.saturating_sub(line.start_index),
        lines: Vec::new(),
    });

    if left < right {
        let mut j = right;
        while j > left && node_points[j - 1].code_point == AsciiCodePoint::DOLLAR_SIGN as i32 {
            j -= 1;
        }
        let count_of_trailing_marker = right - j;
        if count_of_trailing_marker != marker_count {
            return None;
        }

        let lines = vec![PhrasingContentLine {
            node_points: line.node_points.clone(),
            start_index: left,
            end_index: j,
            first_non_whitespace_index: left,
            count_of_precede_spaces: 0,
        }];

        return Some(EatOpenerResult {
            token: token.with_data(MathTokenData {
                marker_count,
                indent: first_non_whitespace_index.saturating_sub(line.start_index),
                lines,
            }),
            next_index: line.end_index,
            saturated: true,
        });
    }

    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: false,
    })
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    let opener = eat_opener(line)?;
    Some(EatAndInterruptPreviousSiblingResult {
        token: opener.token,
        next_index: opener.next_index,
        saturated: opener.saturated,
        remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    let Some(data) = token.data_as::<MathTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    let node_points = line.node_points.as_ref();
    if line.count_of_precede_spaces < 4 && line.first_non_whitespace_index < line.end_index {
        let mut i = line.first_non_whitespace_index;
        while i < line.end_index && node_points[i].code_point == AsciiCodePoint::DOLLAR_SIGN as i32
        {
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

    token.data = std::sync::Arc::new(MathTokenData {
        marker_count: data.marker_count,
        indent: data.indent,
        lines,
    });
    update_token_end_position(token, line);

    EatContinuationTextResult::Opening {
        next_index: line.end_index,
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
