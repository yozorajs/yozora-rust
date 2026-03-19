use std::sync::Arc;

use yozora_ast::{NodeType, Point, Position};
use yozora_character::{calc_trim_boundary_of_code_points, is_space_character, NodePoint};
use yozora_core_tokenizer::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    MatchBlockHook, PhrasingContentLine, RemainingSibling,
};

pub type CheckInfoStringFn =
    Arc<dyn Fn(&[NodePoint], i32, usize) -> bool + Send + Sync + 'static>;

#[derive(Clone)]
pub struct FencedBlockHookContext {
    pub node_type: NodeType,
    pub markers: Vec<i32>,
    pub markers_required: usize,
    pub check_info_string: Option<CheckInfoStringFn>,
}

#[derive(Debug, Clone)]
pub struct FencedBlockTokenData {
    pub marker: i32,
    pub marker_count: usize,
    pub indent: usize,
    pub info_string: Vec<NodePoint>,
    pub lines: Vec<PhrasingContentLine>,
}

pub struct FencedBlockMatchHook {
    context: FencedBlockHookContext,
}

pub fn fenced_block_match(context: FencedBlockHookContext) -> FencedBlockMatchHook {
    FencedBlockMatchHook { context }
}

impl MatchBlockHook for FencedBlockMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        eat_opener(line, &self.context)
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        eat_and_interrupt_previous_sibling(line, prev_sibling_token, &self.context)
    }

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        eat_continuation_text(line, token)
    }
}

pub fn eat_opener(
    line: &PhrasingContentLine,
    context: &FencedBlockHookContext,
) -> Option<EatOpenerResult> {
    // Four spaces indentation produces an indented code block.
    if line.count_of_precede_spaces >= 4 {
        return None;
    }

    let first_non_whitespace_index = line.first_non_whitespace_index;
    if first_non_whitespace_index + context.markers_required.saturating_sub(1) >= line.end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let marker = node_points[first_non_whitespace_index].code_point;
    if !context.markers.contains(&marker) {
        return None;
    }

    let i = eat_optional_characters(node_points, first_non_whitespace_index + 1, line.end_index, marker);
    let marker_count = i - first_non_whitespace_index;
    if marker_count < context.markers_required {
        return None;
    }

    let (left, right) = calc_trim_boundary_of_code_points(node_points, i, line.end_index);
    let info_string = node_points[left..right].to_vec();

    if context
        .check_info_string
        .as_ref()
        .is_some_and(|check| !check(&info_string, marker, marker_count))
    {
        return None;
    }

    let token = BlockToken::new("", context.node_type, calc_line_position(line)).with_data(
        FencedBlockTokenData {
            marker,
            marker_count,
            indent: first_non_whitespace_index.saturating_sub(line.start_index),
            info_string,
            lines: Vec::new(),
        },
    );

    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: false,
    })
}

pub fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
    context: &FencedBlockHookContext,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    let opener = eat_opener(line, context)?;
    Some(EatAndInterruptPreviousSiblingResult {
        token: opener.token,
        next_index: opener.next_index,
        saturated: opener.saturated,
        remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
    })
}

pub fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    let Some(data) = token.data_as::<FencedBlockTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    let node_points = line.node_points.as_ref();

    // Check closing block fence.
    if line.count_of_precede_spaces < 4 && line.first_non_whitespace_index < line.end_index {
        let mut i = eat_optional_characters(
            node_points,
            line.first_non_whitespace_index,
            line.end_index,
            data.marker,
        );
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

    token.data = Arc::new(FencedBlockTokenData {
        marker: data.marker,
        marker_count: data.marker_count,
        indent: data.indent,
        info_string: data.info_string,
        lines,
    });
    update_token_end_position(token, line);

    EatContinuationTextResult::Opening {
        next_index: line.end_index,
    }
}

fn eat_optional_characters(
    node_points: &[NodePoint],
    mut start_index: usize,
    end_index: usize,
    code_point: i32,
) -> usize {
    while start_index < end_index && node_points[start_index].code_point == code_point {
        start_index += 1;
    }
    start_index
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
