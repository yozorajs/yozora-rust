use yozora_ast::{Point, Position, DEFINITION_TYPE};
use yozora_character::{
    calc_string_from_node_points, fold_case, is_ascii_control_character,
    is_whitespace_character, AsciiCodePoint, NodePoint, VirtualCodePoint,
};
use yozora_core_tokenizer::*;

#[derive(Debug, Clone)]
pub(crate) struct LinkLabelCollectingState {
    pub saturated: bool,
    pub node_points: Vec<NodePoint>,
    pub has_non_whitespace_character: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct LinkDestinationCollectingState {
    pub saturated: bool,
    pub node_points: Vec<NodePoint>,
    pub has_open_angle_bracket: bool,
    pub open_parens_count: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct LinkTitleCollectingState {
    pub saturated: bool,
    pub node_points: Vec<NodePoint>,
    pub wrap_symbol: Option<i32>,
}

#[derive(Debug, Clone)]
struct CollectResult<T> {
    next_index: isize,
    state: T,
}

#[derive(Debug, Clone)]
pub(crate) struct TokenData {
    pub lines: Vec<PhrasingContentLine>,
    pub label: LinkLabelCollectingState,
    pub destination: Option<LinkDestinationCollectingState>,
    pub title: Option<LinkTitleCollectingState>,
    pub line_no_of_label: usize,
    pub line_no_of_destination: isize,
    pub line_no_of_title: isize,
}

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.count_of_precede_spaces >= 4 || line.first_non_whitespace_index >= line.end_index {
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
    let mut data = TokenData {
        lines: vec![line.clone()],
        label: label_result.state,
        destination: None,
        title: None,
        line_no_of_label: line_no,
        line_no_of_destination: -1,
        line_no_of_title: -1,
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
    let Some(mut data) = token.data_as::<TokenData>().cloned() else {
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
    token: &BlockToken,
    match_api: &dyn MatchBlockPhaseApi,
) -> Option<OnCloseResult> {
    let data = token.data_as::<TokenData>()?;

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

        let rollback_start_line = calc_title_start_line_index(data);
        let rollback_lines = if rollback_start_line < data.lines.len() {
            data.lines[rollback_start_line..].to_vec()
        } else {
            Vec::new()
        };
        result = Some(OnCloseResult::ClosingAndRollback {
            lines: rollback_lines,
        });
    }

    let (_, identifier) = resolve_label_and_identifier(&data.label.node_points)?;
    match_api.register_definition_identifier(&identifier);
    result
}

fn eat_optional_whitespaces(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> usize {
    let mut i = start_index;
    while i < end_index && is_whitespace_character(node_points[i].code_point) {
        i += 1;
    }
    i
}

fn eat_and_collect_link_label(
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
        next_index: end_index as isize,
        state,
    }
}

fn eat_and_collect_link_destination(
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
                    if i + 1 < end_index {
                        state.node_points.push(point);
                        state.node_points.push(node_points[i + 1]);
                    }
                    i += 2;
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
                if i + 1 < end_index {
                    state.node_points.push(point);
                    state.node_points.push(node_points[i + 1]);
                }
                i += 2;
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
                    state.saturated = true;
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

    state.saturated = true;
    CollectResult {
        next_index: i as isize,
        state,
    }
}

fn eat_and_collect_link_title(
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
                if i + 1 >= end_index
                    || node_points[i + 1].code_point == VirtualCodePoint::LineEnd as i32
                {
                    state.node_points.push(point);
                    state.saturated = true;
                    break;
                }

                return CollectResult {
                    next_index: -1,
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

pub(crate) fn resolve_label_and_identifier(label_points: &[NodePoint]) -> Option<(String, String)> {
    if label_points.len() < 2 {
        return None;
    }

    let label = calc_string_from_node_points(label_points, 1, label_points.len() - 1, false);
    if label.trim().is_empty() {
        return None;
    }

    let mut collapsed = String::new();
    for (idx, part) in label.split_whitespace().enumerate() {
        if idx > 0 {
            collapsed.push(' ');
        }
        collapsed.push_str(part);
    }

    let identifier = fold_case(&collapsed);
    Some((label, identifier))
}

fn calc_title_start_line_index(data: &TokenData) -> usize {
    if data.line_no_of_title < data.line_no_of_label as isize {
        return 0;
    }

    data.line_no_of_title as usize - data.line_no_of_label
}

pub(crate) fn calc_effective_position(token: &BlockToken, data: &TokenData) -> Option<Position> {
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

fn calc_line_end_point(line: &PhrasingContentLine) -> Option<Point> {
    if line.start_index >= line.end_index {
        return None;
    }

    let end = line.node_points[line.end_index - 1];
    Some(Point {
        line: end.line,
        column: end.column + 1,
        offset: Some(end.offset + 1),
    })
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

    let end = line.node_points[line.end_index - 1];
    position.end = Point {
        line: end.line,
        column: end.column + 1,
        offset: Some(end.offset + 1),
    };
}
