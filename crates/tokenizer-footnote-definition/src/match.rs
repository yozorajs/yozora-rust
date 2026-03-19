use yozora_ast::{Point, Position, FOOTNOTE_DEFINITION_TYPE};
use yozora_character::{
    calc_string_from_node_points, is_whitespace_character, AsciiCodePoint, NodePoint,
    VirtualCodePoint,
};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, resolve_label_to_identifier, BlockToken,
    EatContinuationTextResult, EatOpenerResult, MatchBlockPhaseApi, PhrasingContentLine,
};

use crate::parse::FootnoteDefinitionTokenData;

pub(crate) fn eat_opener(
    line: &PhrasingContentLine,
    _api: &dyn MatchBlockPhaseApi,
) -> Option<EatOpenerResult> {
    if line.count_of_precede_spaces >= 4 {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let next_index =
        eat_footnote_label(node_points, line.first_non_whitespace_index, line.end_index);

    if next_index < 0 {
        return None;
    }

    let colon_index = next_index as usize;
    if colon_index >= line.end_index
        || node_points[colon_index].code_point != AsciiCodePoint::COLON as i32
    {
        return None;
    }

    let label = calc_string_from_node_points(
        node_points,
        line.first_non_whitespace_index + 2,
        colon_index - 1,
        true,
    );
    let identifier = resolve_label_to_identifier(&label);

    let token = BlockToken::new(
        "",
        FOOTNOTE_DEFINITION_TYPE,
        calc_line_position(line, colon_index),
    )
    .with_data(FootnoteDefinitionTokenData { label, identifier });

    Some(EatOpenerResult {
        token,
        next_index: colon_index + 1,
        saturated: false,
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
    indent: usize,
) -> EatContinuationTextResult {
    update_token_end_position(token, line);

    if line.first_non_whitespace_index >= line.end_index {
        return EatContinuationTextResult::Opening {
            next_index: std::cmp::min(line.end_index.saturating_sub(1), line.start_index + indent),
        };
    }

    if line.count_of_precede_spaces >= indent {
        return EatContinuationTextResult::Opening {
            next_index: line.start_index + indent,
        };
    }

    EatContinuationTextResult::NotMatched
}

pub(crate) fn on_close(token: &BlockToken, api: &dyn MatchBlockPhaseApi) {
    let Some(data) = token.data_as::<FootnoteDefinitionTokenData>() else {
        return;
    };

    api.registerFootnoteDefinitionIdentifier(&data.identifier);
}

pub fn eat_footnote_label(
    node_points: &[NodePoint],
    first_non_whitespace_index: usize,
    end_index: usize,
) -> isize {
    let mut i = first_non_whitespace_index;

    if i + 1 >= end_index
        || node_points[i].code_point != AsciiCodePoint::OPEN_BRACKET as i32
        || node_points[i + 1].code_point != AsciiCodePoint::CARET as i32
    {
        return -1;
    }

    let mut is_empty = true;
    let last_index = std::cmp::min(end_index, i + 1 + 1000);
    i += 2;

    while i < last_index {
        let code_point = node_points[i].code_point;
        match code_point {
            x if x == AsciiCodePoint::BACKSLASH as i32 => {
                i += 1;
            }
            x if x == AsciiCodePoint::OPEN_BRACKET as i32 => {
                return -1;
            }
            x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
                return if is_empty { -1 } else { (i + 1) as isize };
            }
            x if x == VirtualCodePoint::LineEnd as i32 => {
                return -1;
            }
            _ => {
                if is_empty && !is_whitespace_character(code_point) {
                    is_empty = false;
                }
            }
        }

        i += 1;
    }

    -1
}

fn calc_line_position(line: &PhrasingContentLine, end_index: usize) -> Option<Position> {
    if line.start_index >= line.end_index {
        return None;
    }

    Some(Position {
        start: calc_start_point(line.node_points.as_ref(), line.start_index),
        end: calc_end_point(line.node_points.as_ref(), end_index),
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
