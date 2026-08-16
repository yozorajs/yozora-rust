use yozora_ast::INLINE_MATH_TYPE;
use yozora_character::{is_punctuation_character, is_whitespace_character, AsciiCodePoint};
use yozora_core_tokenizer::{
    eat_optional_characters, DelimiterType, InlineToken, IsDelimiterPairResult,
    MatchInlinePhaseApi, ProcessDelimiterPairResult, TokenDelimiter,
};

use crate::types::InlineMathTokenData;

pub(crate) fn find_delimiter(
    api: &dyn MatchInlinePhaseApi,
    start_index: usize,
    end_index: usize,
) -> Option<TokenDelimiter> {
    let node_points = api.get_node_points();
    let block_start_index = api.get_block_start_index();
    let block_end_index = api.get_block_end_index();

    if start_index >= end_index || end_index > node_points.len() {
        return None;
    }

    let mut i = start_index;
    while i < end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::BACKSLASH as i32 {
            i += 2;
            continue;
        }

        if c != AsciiCodePoint::DOLLAR_SIGN as i32 {
            i += 1;
            continue;
        }

        let delimiter_start_index = i;
        i = eat_optional_characters(
            node_points,
            i + 1,
            block_end_index,
            AsciiCodePoint::DOLLAR_SIGN as i32,
        );

        let left_code_point = if delimiter_start_index == block_start_index {
            None
        } else {
            Some(node_points[delimiter_start_index - 1].code_point)
        };
        let right_code_point = if i == block_end_index {
            None
        } else {
            Some(node_points[i].code_point)
        };

        let thickness = i.saturating_sub(delimiter_start_index);
        let is_potential_opener =
            thickness > 1 || check_if_potential_opener(left_code_point, right_code_point);
        let is_potential_closer =
            thickness > 1 || check_if_potential_closer(left_code_point, right_code_point);
        if !is_potential_opener && !is_potential_closer {
            continue;
        }

        let delimiter_type = if is_potential_opener {
            if is_potential_closer {
                DelimiterType::Both
            } else {
                DelimiterType::Opener
            }
        } else {
            DelimiterType::Closer
        };

        return Some(TokenDelimiter {
            delimiter_type,
            start_index: delimiter_start_index,
            end_index: i,
            thickness,
            original_thickness: thickness,
        });
    }

    None
}

pub(crate) fn is_delimiter_pair(
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
) -> IsDelimiterPairResult {
    if opener_delimiter.thickness == closer_delimiter.thickness
        && matches!(
            opener_delimiter.delimiter_type,
            DelimiterType::Opener | DelimiterType::Both
        )
        && matches!(
            closer_delimiter.delimiter_type,
            DelimiterType::Closer | DelimiterType::Both
        )
    {
        IsDelimiterPairResult::Paired
    } else {
        IsDelimiterPairResult::NotPaired {
            opener: true,
            closer: true,
        }
    }
}

pub(crate) fn process_delimiter_pair(
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
) -> ProcessDelimiterPairResult {
    let thickness = opener_delimiter.thickness;

    ProcessDelimiterPairResult {
        tokens: vec![InlineToken::new(
            "",
            INLINE_MATH_TYPE,
            (opener_delimiter.start_index, closer_delimiter.end_index),
        )
        .with_data(InlineMathTokenData { thickness })],
        remain_opener_delimiter: None,
        remain_closer_delimiter: None,
    }
}

fn check_if_potential_opener(left_code_point: Option<i32>, right_code_point: Option<i32>) -> bool {
    if right_code_point.is_none() {
        return false;
    }

    if left_code_point == Some(AsciiCodePoint::BACKTICK as i32) {
        return false;
    }
    let Some(left) = left_code_point else {
        return true;
    };

    is_whitespace_character(left) || is_punctuation_character(left)
}

fn check_if_potential_closer(left_code_point: Option<i32>, right_code_point: Option<i32>) -> bool {
    if left_code_point.is_none() {
        return false;
    }

    if right_code_point == Some(AsciiCodePoint::BACKTICK as i32) {
        return false;
    }
    let Some(right) = right_code_point else {
        return true;
    };

    is_whitespace_character(right) || is_punctuation_character(right)
}
