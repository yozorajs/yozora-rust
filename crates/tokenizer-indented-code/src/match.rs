use yozora_ast::{Point, Position, CODE_TYPE};
use yozora_character::{AsciiCodePoint, VirtualCodePoint};
use yozora_core_tokenizer::{
    BlockToken, EatContinuationTextResult, EatOpenerResult, PhrasingContentLine,
};

use crate::parse::IndentedCodeTokenData;

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.count_of_precede_spaces < 4 {
        return None;
    }

    let mut first_index = line.start_index + 4;
    if line.start_index + 3 < line.node_points.len()
        && line.node_points[line.start_index].code_point == AsciiCodePoint::SPACE as i32
        && line.node_points[line.start_index + 3].code_point == VirtualCodePoint::Space as i32
    {
        let mut i = line.start_index + 1;
        while i < line.first_non_whitespace_index {
            if line.node_points[i].code_point == VirtualCodePoint::Space as i32 {
                break;
            }
            i += 1;
        }
        first_index = i + 4;
    }

    let token =
        BlockToken::new("", CODE_TYPE, calc_line_position(line)).with_data(IndentedCodeTokenData {
            lines: vec![PhrasingContentLine {
                node_points: line.node_points.clone(),
                start_index: first_index,
                end_index: line.end_index,
                first_non_whitespace_index: line.first_non_whitespace_index,
                count_of_precede_spaces: line
                    .count_of_precede_spaces
                    .saturating_sub(first_index.saturating_sub(line.start_index)),
            }],
        });

    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: false,
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    let Some(data) = token.data_as::<IndentedCodeTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    if line.count_of_precede_spaces < 4 && line.first_non_whitespace_index < line.end_index {
        return EatContinuationTextResult::NotMatched;
    }

    let first_index = std::cmp::min(line.end_index.saturating_sub(1), line.start_index + 4);
    let mut lines = data.lines;
    lines.push(PhrasingContentLine {
        node_points: line.node_points.clone(),
        start_index: first_index,
        end_index: line.end_index,
        first_non_whitespace_index: line.first_non_whitespace_index,
        count_of_precede_spaces: line
            .count_of_precede_spaces
            .saturating_sub(first_index.saturating_sub(line.start_index)),
    });

    token.data = std::sync::Arc::new(IndentedCodeTokenData { lines });
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
