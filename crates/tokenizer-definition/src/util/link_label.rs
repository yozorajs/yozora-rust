use yozora_character::{is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::eat_optional_whitespaces;

use crate::types::{CollectResult, LinkLabelCollectingState};

const MAX_COLLECTED_LINK_LABEL_LENGTH: usize = 1000;

pub fn eat_and_collect_link_label(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    state: Option<LinkLabelCollectingState>,
) -> CollectResult<LinkLabelCollectingState> {
    let mut i = start_index;
    let mut state = state.unwrap_or(LinkLabelCollectingState {
        saturated: false,
        node_points: Vec::new(),
        has_non_whitespace_character: false,
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
        if node_points[i].code_point != AsciiCodePoint::OPEN_BRACKET as i32 {
            return CollectResult {
                next_index: -1,
                state,
            };
        }
        state.node_points.push(node_points[i]);
        i += 1;
    }

    while i < end_index {
        if state.node_points.len() > MAX_COLLECTED_LINK_LABEL_LENGTH {
            return CollectResult {
                next_index: -1,
                state,
            };
        }

        let point = node_points[i];
        match point.code_point {
            x if x == AsciiCodePoint::BACKSLASH as i32 => {
                state.has_non_whitespace_character = true;
                if i + 1 < end_index {
                    state.node_points.push(point);
                    state.node_points.push(node_points[i + 1]);
                }
                i += 2;
            }
            x if x == AsciiCodePoint::OPEN_BRACKET as i32 => {
                return CollectResult {
                    next_index: -1,
                    state,
                };
            }
            x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
                state.node_points.push(point);
                if state.has_non_whitespace_character {
                    state.saturated = true;
                    return CollectResult {
                        next_index: (i + 1) as isize,
                        state,
                    };
                }
                return CollectResult {
                    next_index: -1,
                    state,
                };
            }
            _ => {
                if !is_whitespace_character(point.code_point) {
                    state.has_non_whitespace_character = true;
                }
                state.node_points.push(point);
                i += 1;
            }
        }
    }

    CollectResult {
        next_index: 1,
        state,
    }
}
