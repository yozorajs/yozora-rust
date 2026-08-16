use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::eat_optional_whitespaces;

use crate::types::{CollectResult, LinkTitleCollectingState};

pub fn eat_and_collect_link_title(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    state: Option<LinkTitleCollectingState>,
) -> CollectResult<LinkTitleCollectingState> {
    let mut i = start_index;
    let mut state = state.unwrap_or(LinkTitleCollectingState {
        saturated: false,
        node_points: Vec::new(),
        wrap_symbol: None,
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
        match node_points[i].code_point {
            x if x == AsciiCodePoint::DOUBLE_QUOTE as i32
                || x == AsciiCodePoint::SINGLE_QUOTE as i32
                || x == AsciiCodePoint::OPEN_PARENTHESIS as i32 =>
            {
                state.wrap_symbol = Some(node_points[i].code_point);
                state.node_points.push(node_points[i]);
                i += 1;
            }
            _ => {
                return CollectResult {
                    next_index: -1,
                    state,
                };
            }
        }
    }

    let Some(wrap_symbol) = state.wrap_symbol else {
        return CollectResult {
            next_index: -1,
            state,
        };
    };

    if wrap_symbol == AsciiCodePoint::DOUBLE_QUOTE as i32
        || wrap_symbol == AsciiCodePoint::SINGLE_QUOTE as i32
    {
        while i < end_index {
            let point = node_points[i];
            match point.code_point {
                x if x == AsciiCodePoint::BACKSLASH as i32 => {
                    if i + 1 < end_index {
                        state.node_points.push(point);
                        state.node_points.push(node_points[i + 1]);
                    }
                    i += 2;
                }
                x if x == wrap_symbol => {
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
            next_index: end_index as isize,
            state,
        };
    }

    while i < end_index {
        let point = node_points[i];
        match point.code_point {
            x if x == AsciiCodePoint::BACKSLASH as i32 => {
                if i + 1 < end_index {
                    state.node_points.push(point);
                    state.node_points.push(node_points[i + 1]);
                }
                i += 2;
            }
            x if x == AsciiCodePoint::OPEN_PARENTHESIS as i32 => {
                return CollectResult {
                    next_index: -1,
                    state,
                };
            }
            x if x == AsciiCodePoint::CLOSE_PARENTHESIS as i32 => {
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

    CollectResult {
        next_index: end_index as isize,
        state,
    }
}
