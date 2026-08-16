use yozora_ast::{Position, FOOTNOTE_DEFINITION_TYPE};
use yozora_character::{calc_string_from_node_points, AsciiCodePoint};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, eat_indentation, resolve_label_to_identifier, BlockToken,
    EatContinuationTextResult, EatOpenerResult, MatchBlockPhaseApi, PhrasingContentLine,
};

use crate::types::{FootnoteDefinitionLabel, FootnoteDefinitionTokenData};
use crate::util::eat_footnote_label;

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width >= 4 {
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

    let token = BlockToken::new(
        "",
        FOOTNOTE_DEFINITION_TYPE,
        calc_line_position(line, colon_index),
    )
    .with_data(FootnoteDefinitionTokenData {
        label: FootnoteDefinitionLabel {
            node_points: line.node_points.clone(),
            start_index: line.first_non_whitespace_index,
            end_index: colon_index,
        },
        _label: None,
        _identifier: None,
    });

    Some(EatOpenerResult {
        token,
        next_index: colon_index + 1,
        saturated: false,
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    _token: &mut BlockToken,
    indent: usize,
) -> EatContinuationTextResult {
    if line.first_non_whitespace_index >= line.end_index {
        return EatContinuationTextResult::Opening {
            next_index: std::cmp::min(line.end_index.saturating_sub(1), line.start_index + indent),
        };
    }

    if line.indent_width < indent {
        return EatContinuationTextResult::NotMatched;
    }

    let Some(next_index) = eat_indentation(
        line.node_points.as_ref(),
        line.start_index,
        line.first_non_whitespace_index,
        indent,
    ) else {
        return EatContinuationTextResult::NotMatched;
    };

    EatContinuationTextResult::Opening { next_index }
}

pub(crate) fn on_close(token: &mut BlockToken, api: &dyn MatchBlockPhaseApi) {
    let Some(mut data) = token.data_as::<FootnoteDefinitionTokenData>().cloned() else {
        return;
    };

    let label = calc_string_from_node_points(
        data.label.node_points.as_ref(),
        data.label.start_index + 2,
        data.label.end_index - 1,
        false,
    );
    let identifier = resolve_label_to_identifier(&label);
    api.register_footnote_definition_identifier(&identifier);
    data._label = Some(label);
    data._identifier = Some(identifier);
    token.data = std::sync::Arc::new(data);
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
