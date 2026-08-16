use std::sync::Arc;

use yozora_ast::HTML_TYPE;
use yozora_character::{calc_string_from_node_points, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, eat_optional_whitespaces, BlockToken,
    EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    PhrasingContentLine, RemainingSibling,
};

use crate::conditions::c1::{eat_end_condition1, eat_start_condition1};
use crate::conditions::c2::{eat_end_condition2, eat_start_condition2};
use crate::conditions::c3::{eat_end_condition3, eat_start_condition3};
use crate::conditions::c4::{eat_end_condition4, eat_start_condition4};
use crate::conditions::c5::{eat_end_condition5, eat_start_condition5};
use crate::conditions::c6::eat_start_condition6;
use crate::conditions::c7::eat_start_condition7;
use crate::types::{HtmlBlockConditionType, HtmlBlockTokenData};
use crate::util::eat_html_tag_name;

struct StartConditionResult {
    condition: HtmlBlockConditionType,
    next_index: usize,
}

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width >= 4 {
        return None;
    }
    let node_points = line.node_points.as_ref();
    if line.first_non_whitespace_index >= line.end_index
        || node_points[line.first_non_whitespace_index].code_point
            != AsciiCodePoint::OPEN_ANGLE as i32
    {
        return None;
    }

    let start_result = eat_start_condition(
        node_points,
        line.first_non_whitespace_index + 1,
        line.end_index,
    )?;
    let saturated = !matches!(
        start_result.condition,
        HtmlBlockConditionType::Condition6 | HtmlBlockConditionType::Condition7
    ) && eat_end_condition(
        node_points,
        start_result.next_index,
        line.end_index,
        start_result.condition,
    )
    .is_some();

    let token =
        BlockToken::new("", HTML_TYPE, calc_line_position(line)).with_data(HtmlBlockTokenData {
            condition: start_result.condition,
            lines: vec![line.clone()],
        });
    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated,
    })
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    let result = eat_opener(line)?;
    let data = result.token.data_as::<HtmlBlockTokenData>()?;
    if data.condition == HtmlBlockConditionType::Condition7 {
        return None;
    }
    Some(EatAndInterruptPreviousSiblingResult {
        token: result.token,
        next_index: result.next_index,
        remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
        saturated: result.saturated,
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    let Some(mut data) = token.data_as::<HtmlBlockTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };
    let next_index = eat_end_condition(
        line.node_points.as_ref(),
        line.first_non_whitespace_index,
        line.end_index,
        data.condition,
    );
    if next_index == Some(-1) {
        return EatContinuationTextResult::NotMatched;
    }

    data.lines.push(line.clone());
    token.data = Arc::new(data);
    update_token_end_position(token, line);
    if next_index.is_some() {
        EatContinuationTextResult::Closing {
            next_index: line.end_index,
        }
    } else {
        EatContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

fn eat_start_condition(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<StartConditionResult> {
    if start_index >= end_index {
        return None;
    }
    for (condition, next_index) in [
        (
            HtmlBlockConditionType::Condition2,
            eat_start_condition2(node_points, start_index, end_index),
        ),
        (
            HtmlBlockConditionType::Condition3,
            eat_start_condition3(node_points, start_index, end_index),
        ),
        (
            HtmlBlockConditionType::Condition4,
            eat_start_condition4(node_points, start_index, end_index),
        ),
        (
            HtmlBlockConditionType::Condition5,
            eat_start_condition5(node_points, start_index, end_index),
        ),
    ] {
        if let Some(next_index) = next_index {
            return Some(StartConditionResult {
                condition,
                next_index,
            });
        }
    }

    let potential_open_tag = node_points[start_index].code_point != AsciiCodePoint::SLASH as i32;
    let tag_name_start_index = start_index + usize::from(!potential_open_tag);
    let tag_name_end_index = eat_html_tag_name(node_points, tag_name_start_index, end_index)?;
    let tag_name =
        calc_string_from_node_points(node_points, tag_name_start_index, tag_name_end_index, false)
            .to_lowercase();

    if potential_open_tag {
        if let Some(next_index) =
            eat_start_condition1(node_points, tag_name_end_index, end_index, &tag_name)
        {
            return Some(StartConditionResult {
                condition: HtmlBlockConditionType::Condition1,
                next_index,
            });
        }
    }
    if let Some(next_index) =
        eat_start_condition6(node_points, tag_name_end_index, end_index, &tag_name)
    {
        return Some(StartConditionResult {
            condition: HtmlBlockConditionType::Condition6,
            next_index,
        });
    }
    eat_start_condition7(
        node_points,
        tag_name_end_index,
        end_index,
        &tag_name,
        potential_open_tag,
    )
    .map(|next_index| StartConditionResult {
        condition: HtmlBlockConditionType::Condition7,
        next_index,
    })
}

fn eat_end_condition(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    condition: HtmlBlockConditionType,
) -> Option<isize> {
    let matched = match condition {
        HtmlBlockConditionType::Condition1 => {
            eat_end_condition1(node_points, start_index, end_index)
        }
        HtmlBlockConditionType::Condition2 => {
            eat_end_condition2(node_points, start_index, end_index)
        }
        HtmlBlockConditionType::Condition3 => {
            eat_end_condition3(node_points, start_index, end_index)
        }
        HtmlBlockConditionType::Condition4 => {
            eat_end_condition4(node_points, start_index, end_index)
        }
        HtmlBlockConditionType::Condition5 => {
            eat_end_condition5(node_points, start_index, end_index)
        }
        HtmlBlockConditionType::Condition6 | HtmlBlockConditionType::Condition7 => {
            return (eat_optional_whitespaces(node_points, start_index, end_index) >= end_index)
                .then_some(-1);
        }
    };
    matched.map(|_| end_index as isize)
}

fn calc_line_position(line: &PhrasingContentLine) -> Option<yozora_ast::Position> {
    if line.start_index >= line.end_index {
        return None;
    }
    Some(yozora_ast::Position {
        start: calc_start_point(line.node_points.as_ref(), line.start_index),
        end: calc_end_point(line.node_points.as_ref(), line.end_index - 1),
        indent: None,
    })
}

fn update_token_end_position(token: &mut BlockToken, line: &PhrasingContentLine) {
    let Some(position) = token.position.as_mut() else {
        return;
    };
    if line.start_index < line.end_index {
        position.end = calc_end_point(line.node_points.as_ref(), line.end_index - 1);
    }
}
