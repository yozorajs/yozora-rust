use yozora_character::{
    is_ascii_control_character, is_ascii_punctuation_character, is_whitespace_character,
    AsciiCodePoint, NodePoint, VirtualCodePoint,
};
use yozora_core_tokenizer::eat_optional_whitespaces;

use crate::types::{CollectResult, LinkDestinationCollectingState};

pub fn eat_and_collect_link_destination(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    state: Option<LinkDestinationCollectingState>,
) -> CollectResult<LinkDestinationCollectingState> {
    let mut i = start_index;
    let mut state = state.unwrap_or(LinkDestinationCollectingState {
        saturated: false,
        node_points: Vec::new(),
        has_open_angle_bracket: false,
        open_parens_count: 0,
    });

    let first_non_whitespace_index = eat_optional_whitespaces(node_points, i, end_index);
    if first_non_whitespace_index >= end_index {
        return CollectResult {
            next_index: -1,
            state,
        };
    }

    if state.node_points.is_empty() {
        i = first_non_whitespace_index;
        if node_points[i].code_point == AsciiCodePoint::OPEN_ANGLE as i32 {
            state.has_open_angle_bracket = true;
            state.node_points.push(node_points[i]);
            i += 1;
        }
    }

    if state.has_open_angle_bracket {
        while i < end_index {
            let point = node_points[i];
            match point.code_point {
                x if x == AsciiCodePoint::BACKSLASH as i32 => {
                    state.node_points.push(point);
                    if i + 1 < end_index
                        && is_ascii_punctuation_character(node_points[i + 1].code_point)
                    {
                        state.node_points.push(node_points[i + 1]);
                        i += 1;
                    }
                    i += 1;
                }
                x if x == AsciiCodePoint::OPEN_ANGLE as i32
                    || x == VirtualCodePoint::LineEnd as i32 =>
                {
                    return CollectResult {
                        next_index: -1,
                        state,
                    };
                }
                x if x == AsciiCodePoint::CLOSE_ANGLE as i32 => {
                    state.saturated = true;
                    state.node_points.push(point);
                    return CollectResult {
                        next_index: (i + 1) as isize,
                        state,
                    };
                }
                _ => {
                    state.node_points.push(point);
                    i += 1;
                }
            }
        }
        return CollectResult {
            next_index: i as isize,
            state,
        };
    }

    while i < end_index {
        let point = node_points[i];
        match point.code_point {
            x if x == AsciiCodePoint::BACKSLASH as i32 => {
                state.node_points.push(point);
                if i + 1 < end_index
                    && is_ascii_punctuation_character(node_points[i + 1].code_point)
                {
                    state.node_points.push(node_points[i + 1]);
                    i += 1;
                }
                i += 1;
            }
            x if x == AsciiCodePoint::OPEN_PARENTHESIS as i32 => {
                state.open_parens_count += 1;
                state.node_points.push(point);
                i += 1;
            }
            x if x == AsciiCodePoint::CLOSE_PARENTHESIS as i32 => {
                state.open_parens_count -= 1;
                state.node_points.push(point);
                if state.open_parens_count < 0 {
                    return CollectResult {
                        next_index: i as isize,
                        state,
                    };
                }
                i += 1;
            }
            _ => {
                if is_whitespace_character(point.code_point)
                    || is_ascii_control_character(point.code_point)
                {
                    state.saturated = state.open_parens_count == 0;
                    return CollectResult {
                        next_index: i as isize,
                        state,
                    };
                }
                state.node_points.push(point);
                i += 1;
            }
        }
    }

    state.saturated = state.open_parens_count == 0;
    CollectResult {
        next_index: i as isize,
        state,
    }
}
