use yozora_ast::{Position, CODE_TYPE};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, eat_indentation, BlockToken, EatContinuationTextResult,
    EatOpenerResult, PhrasingContentLine,
};

use crate::types::IndentedCodeTokenData;

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width < 4 {
        return None;
    }

    let first_index = eat_indentation(
        line.node_points.as_ref(),
        line.start_index,
        line.first_non_whitespace_index,
        4,
    )?;

    let token =
        BlockToken::new("", CODE_TYPE, calc_line_position(line)).with_data(IndentedCodeTokenData {
            lines: vec![PhrasingContentLine {
                node_points: line.node_points.clone(),
                start_index: first_index,
                end_index: line.end_index,
                first_non_whitespace_index: line.first_non_whitespace_index,
                indent_width: line.indent_width.saturating_sub(4),
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

    if line.indent_width < 4 && line.first_non_whitespace_index < line.end_index {
        return EatContinuationTextResult::NotMatched;
    }

    let first_index = if line.first_non_whitespace_index < line.end_index {
        let Some(first_index) = eat_indentation(
            line.node_points.as_ref(),
            line.start_index,
            line.first_non_whitespace_index,
            4,
        ) else {
            return EatContinuationTextResult::NotMatched;
        };
        first_index
    } else {
        std::cmp::min(line.end_index.saturating_sub(1), line.start_index + 4)
    };
    let mut lines = data.lines;
    lines.push(PhrasingContentLine {
        node_points: line.node_points.clone(),
        start_index: first_index,
        end_index: line.end_index,
        first_non_whitespace_index: line.first_non_whitespace_index,
        indent_width: line.indent_width.saturating_sub(4),
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

    Some(Position {
        start: calc_start_point(line.node_points.as_ref(), line.start_index),
        end: calc_end_point(line.node_points.as_ref(), line.end_index - 1),
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
