use yozora_ast::INLINE_MATH_TYPE;
use yozora_character::{
    is_punctuation_character, is_whitespace_character, AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::{
    DelimiterType, InlineToken, IsDelimiterPairResult, MatchInlinePhaseApi,
    ProcessDelimiterPairResult, TokenDelimiter,
};

use crate::parse::InlineMathTokenData;

#[derive(Debug, Clone)]
struct PotentialDelimiter {
    delimiter_type: DelimiterType,
    start_index: usize,
    end_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct InlineMathBacktickDelimiterFinder {
    potential_delimiters: Vec<PotentialDelimiter>,
    cursor: usize,
}

impl InlineMathBacktickDelimiterFinder {
    pub(crate) fn new(api: &dyn MatchInlinePhaseApi) -> Self {
        let potential_delimiters = collect_potential_delimiters(
            api.get_node_points(),
            api.get_block_start_index(),
            api.get_block_end_index(),
        );

        Self {
            potential_delimiters,
            cursor: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.cursor = 0;
    }

    pub(crate) fn find_next_delimiter(&mut self, start_index: usize) -> Option<TokenDelimiter> {
        let len = self.potential_delimiters.len();
        while self.cursor < len {
            while self.cursor < len {
                let delimiter = &self.potential_delimiters[self.cursor];
                if delimiter.start_index >= start_index
                    && delimiter.delimiter_type != DelimiterType::Closer
                {
                    break;
                }
                self.cursor += 1;
            }

            if self.cursor + 1 >= len {
                return None;
            }

            let opener = &self.potential_delimiters[self.cursor];
            let thickness = opener.end_index.saturating_sub(opener.start_index);

            let mut closer: Option<&PotentialDelimiter> = None;
            for i in (self.cursor + 1)..len {
                let delimiter = &self.potential_delimiters[i];
                if delimiter.delimiter_type != DelimiterType::Opener
                    && delimiter.end_index.saturating_sub(delimiter.start_index) == thickness
                {
                    closer = Some(delimiter);
                    break;
                }
            }

            if let Some(closer) = closer {
                return Some(TokenDelimiter {
                    delimiter_type: DelimiterType::Full,
                    start_index: opener.start_index,
                    end_index: closer.end_index,
                    thickness,
                    original_thickness: thickness,
                });
            }

            self.cursor += 1;
        }

        None
    }
}

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
        remainOpenerDelimiter: None,
        remainCloserDelimiter: None,
    }
}

pub(crate) fn process_single_delimiter(delimiter: &TokenDelimiter) -> Vec<InlineToken> {
    vec![InlineToken::new(
        "",
        INLINE_MATH_TYPE,
        (delimiter.start_index, delimiter.end_index),
    )
    .with_data(InlineMathTokenData {
        thickness: delimiter.thickness,
    })]
}

fn collect_potential_delimiters(
    node_points: &[NodePoint],
    block_start_index: usize,
    block_end_index: usize,
) -> Vec<PotentialDelimiter> {
    let mut delimiters = Vec::new();
    let mut i = block_start_index;

    while i < block_end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::BACKSLASH as i32 {
            i += 2;
            continue;
        }

        if c == AsciiCodePoint::BACKTICK as i32 {
            let start_index = i;
            let j = eat_optional_characters(
                node_points,
                i + 1,
                block_end_index,
                AsciiCodePoint::BACKTICK as i32,
            );

            if j < block_end_index
                && node_points[j].code_point == AsciiCodePoint::DOLLAR_SIGN as i32
            {
                delimiters.push(PotentialDelimiter {
                    delimiter_type: DelimiterType::Opener,
                    start_index,
                    end_index: j + 1,
                });
            }

            i = j;
            continue;
        }

        if c == AsciiCodePoint::DOLLAR_SIGN as i32 {
            let start_index = i;
            let j = eat_optional_characters(
                node_points,
                i + 1,
                block_end_index,
                AsciiCodePoint::BACKTICK as i32,
            );
            let thickness = j.saturating_sub(start_index);

            if thickness > 1
                && (j >= block_end_index
                    || node_points[j].code_point != AsciiCodePoint::DOLLAR_SIGN as i32)
            {
                delimiters.push(PotentialDelimiter {
                    delimiter_type: DelimiterType::Closer,
                    start_index,
                    end_index: j,
                });
            }

            i = j;
            continue;
        }

        i += 1;
    }

    delimiters
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
