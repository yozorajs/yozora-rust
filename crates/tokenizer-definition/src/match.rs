use yozora_ast::{Point, Position, DEFINITION_TYPE};
use yozora_character::{calc_string_from_node_points, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::*;

use crate::types::DefinitionTokenData;
use crate::util::{
    eat_and_collect_link_destination, eat_and_collect_link_label, eat_and_collect_link_title,
};

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width >= 4 || line.first_non_whitespace_index >= line.end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let start_index = line.start_index;
    let end_index = line.end_index;

    let label_result = eat_and_collect_link_label(
        node_points,
        line.first_non_whitespace_index,
        end_index,
        None,
    );
    if label_result.next_index < 0 {
        return None;
    }

    let line_no = node_points[start_index].line;
    let mut data = DefinitionTokenData {
        lines: vec![line.clone()],
        label: label_result.state,
        destination: None,
        title: None,
        line_no_of_label: line_no,
        line_no_of_destination: -1,
        line_no_of_title: -1,
        _label: None,
        _identifier: None,
    };

    if !data.label.saturated {
        let token = BlockToken::new("", DEFINITION_TYPE, calc_line_position(line)).with_data(data);
        return Some(EatOpenerResult {
            token,
            next_index: end_index,
            saturated: false,
        });
    }

    let label_end_index = label_result.next_index as usize;
    if label_end_index + 1 >= end_index
        || node_points[label_end_index].code_point != AsciiCodePoint::COLON as i32
    {
        return None;
    }

    let mut i = eat_optional_whitespaces(node_points, label_end_index + 1, end_index);
    if i >= end_index {
        let token = BlockToken::new("", DEFINITION_TYPE, calc_line_position(line)).with_data(data);
        return Some(EatOpenerResult {
            token,
            next_index: end_index,
            saturated: false,
        });
    }

    let destination_result = eat_and_collect_link_destination(node_points, i, end_index, None);
    if destination_result.next_index < 0 {
        return None;
    }

    let destination_end_index = destination_result.next_index as usize;
    if !destination_result.state.saturated && destination_end_index != end_index {
        return None;
    }

    i = eat_optional_whitespaces(node_points, destination_end_index, end_index);
    if i >= end_index {
        data.destination = Some(destination_result.state);
        data.line_no_of_destination = line_no as isize;

        let token = BlockToken::new("", DEFINITION_TYPE, calc_line_position(line)).with_data(data);
        return Some(EatOpenerResult {
            token,
            next_index: end_index,
            saturated: false,
        });
    }

    if i == destination_end_index {
        return None;
    }

    let title_result = eat_and_collect_link_title(node_points, i, end_index, None);
    if title_result.next_index >= 0 {
        i = title_result.next_index as usize;
    }

    if i < end_index {
        let k = eat_optional_whitespaces(node_points, i, end_index);
        if k < end_index {
            return None;
        }
    }

    data.destination = Some(destination_result.state);
    data.title = Some(title_result.state);
    data.line_no_of_destination = line_no as isize;
    data.line_no_of_title = line_no as isize;

    let token = BlockToken::new("", DEFINITION_TYPE, calc_line_position(line)).with_data(data);
    Some(EatOpenerResult {
        token,
        next_index: end_index,
        saturated: false,
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    let Some(mut data) = token.data_as::<DefinitionTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    if data.title.as_ref().is_some_and(|title| title.saturated) {
        return EatContinuationTextResult::NotMatched;
    }

    let node_points = line.node_points.as_ref();
    let start_index = line.start_index;
    let end_index = line.end_index;
    let line_no = node_points[start_index].line;

    let mut i = line.first_non_whitespace_index;
    if !data.label.saturated {
        let label_result =
            eat_and_collect_link_label(node_points, i, end_index, Some(data.label.clone()));
        if label_result.next_index < 0 {
            return EatContinuationTextResult::FailedAndRollback {
                lines: data.lines.clone(),
            };
        }

        data.label = label_result.state;
        if !data.label.saturated {
            data.lines.push(line.clone());
            token.data = std::sync::Arc::new(data);
            update_token_end_position(token, line);
            return EatContinuationTextResult::Opening {
                next_index: end_index,
            };
        }

        let label_end_index = label_result.next_index as usize;
        if label_end_index + 1 >= end_index
            || node_points[label_end_index].code_point != AsciiCodePoint::COLON as i32
        {
            return EatContinuationTextResult::FailedAndRollback {
                lines: data.lines.clone(),
            };
        }

        i = label_end_index + 1;
    }

    if data.destination.is_none() {
        i = eat_optional_whitespaces(node_points, i, end_index);
        if i >= end_index {
            return EatContinuationTextResult::FailedAndRollback {
                lines: data.lines.clone(),
            };
        }

        let destination_result = eat_and_collect_link_destination(node_points, i, end_index, None);
        if destination_result.next_index < 0 || !destination_result.state.saturated {
            return EatContinuationTextResult::FailedAndRollback {
                lines: data.lines.clone(),
            };
        }

        let destination_end_index = destination_result.next_index as usize;
        i = eat_optional_whitespaces(node_points, destination_end_index, end_index);
        if i >= end_index {
            data.destination = Some(destination_result.state);
            data.line_no_of_destination = line_no as isize;
            data.lines.push(line.clone());

            token.data = std::sync::Arc::new(data);
            update_token_end_position(token, line);
            return EatContinuationTextResult::Opening {
                next_index: end_index,
            };
        }

        data.destination = Some(destination_result.state);
        data.line_no_of_destination = line_no as isize;
        data.line_no_of_title = line_no as isize;
    }

    if data.line_no_of_title < 0 {
        data.line_no_of_title = line_no as isize;
    }

    let title_result = eat_and_collect_link_title(node_points, i, end_index, data.title.clone());
    let title_end_index = title_result.next_index;
    let title_state = title_result.state;
    let title_saturated = title_state.saturated;
    let title_has_content = !title_state.node_points.is_empty();
    data.title = Some(title_state);

    let has_invalid_tail = if title_end_index >= 0 && title_saturated {
        eat_optional_whitespaces(node_points, title_end_index as usize, end_index) < end_index
    } else {
        false
    };

    if title_end_index < 0 || !title_has_content || has_invalid_tail {
        if data.line_no_of_destination == data.line_no_of_title {
            return EatContinuationTextResult::FailedAndRollback {
                lines: data.lines.clone(),
            };
        }

        data.title = None;

        let rollback_start_line = calc_title_start_line_index(&data);
        if rollback_start_line > 0 {
            if let Some(kept_last_line) = data.lines.get(rollback_start_line - 1) {
                set_token_end_position_from_line(token, kept_last_line);
            }
        }

        let rollback_lines = if rollback_start_line < data.lines.len() {
            data.lines[rollback_start_line..].to_vec()
        } else {
            Vec::new()
        };
        data.lines.truncate(rollback_start_line);

        token.data = std::sync::Arc::new(data);
        return EatContinuationTextResult::ClosingAndRollback {
            lines: rollback_lines,
        };
    }

    data.lines.push(line.clone());

    token.data = std::sync::Arc::new(data);
    update_token_end_position(token, line);
    if title_saturated {
        EatContinuationTextResult::Closing {
            next_index: end_index,
        }
    } else {
        EatContinuationTextResult::Opening {
            next_index: end_index,
        }
    }
}

pub(crate) fn on_close(
    token: &mut BlockToken,
    match_api: &dyn MatchBlockPhaseApi,
) -> Option<OnCloseResult> {
    let mut data = token.data_as::<DefinitionTokenData>()?.clone();

    if !data.label.saturated {
        return Some(OnCloseResult::FailedAndRollback {
            lines: data.lines.clone(),
        });
    }

    let Some(destination) = &data.destination else {
        return Some(OnCloseResult::FailedAndRollback {
            lines: data.lines.clone(),
        });
    };
    if !destination.saturated {
        return Some(OnCloseResult::FailedAndRollback {
            lines: data.lines.clone(),
        });
    }

    let mut result = None;
    if data.title.as_ref().is_some_and(|title| !title.saturated) {
        if data.line_no_of_destination == data.line_no_of_title {
            return Some(OnCloseResult::FailedAndRollback {
                lines: data.lines.clone(),
            });
        }

        let rollback_start_line = calc_title_start_line_index(&data);
        let rollback_lines = if rollback_start_line < data.lines.len() {
            data.lines[rollback_start_line..].to_vec()
        } else {
            Vec::new()
        };
        result = Some(OnCloseResult::ClosingAndRollback {
            lines: rollback_lines,
        });
    }

    let (label, identifier) = resolve_label_and_identifier(&data.label.node_points)?;
    match_api.register_definition_identifier(&identifier);
    data._label = Some(label);
    data._identifier = Some(identifier);
    token.data = std::sync::Arc::new(data);
    result
}

pub(crate) fn resolve_label_and_identifier(label_points: &[NodePoint]) -> Option<(String, String)> {
    if label_points.len() < 2 {
        return None;
    }

    let label = calc_string_from_node_points(label_points, 1, label_points.len() - 1, false);
    if label.trim().is_empty() {
        return None;
    }

    let identifier = resolve_label_to_identifier(&label);
    Some((label, identifier))
}

fn calc_title_start_line_index(data: &DefinitionTokenData) -> usize {
    if data.line_no_of_title < data.line_no_of_label as isize {
        return 0;
    }

    data.line_no_of_title as usize - data.line_no_of_label
}

pub(crate) fn calc_effective_position(
    token: &BlockToken,
    data: &DefinitionTokenData,
) -> Option<Position> {
    let mut position = token.position.clone()?;

    if data.title.as_ref().is_some_and(|title| !title.saturated)
        && data.line_no_of_destination >= 0
        && data.line_no_of_destination < data.line_no_of_title
    {
        let title_start_line = calc_title_start_line_index(data);
        if title_start_line > 0 {
            if let Some(kept_last_line) = data.lines.get(title_start_line - 1) {
                if let Some(point) = calc_line_end_point(kept_last_line) {
                    position.end = point;
                }
            }
        }
    }

    Some(position)
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

fn calc_line_end_point(line: &PhrasingContentLine) -> Option<Point> {
    if line.start_index >= line.end_index {
        return None;
    }

    Some(calc_end_point(
        line.node_points.as_ref(),
        line.end_index - 1,
    ))
}

fn set_token_end_position_from_line(token: &mut BlockToken, line: &PhrasingContentLine) {
    let Some(position) = token.position.as_mut() else {
        return;
    };

    if let Some(end) = calc_line_end_point(line) {
        position.end = end;
    }
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
