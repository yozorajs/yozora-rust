use yozora_ast::{EMPHASIS_TYPE, STRONG_TYPE};
use yozora_character::{
    is_punctuation_character, is_unicode_whitespace_character, AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::*;

use crate::parse::EmphasisTokenData;

pub(crate) fn find_delimiter(
    api: &dyn MatchInlinePhaseApi,
    start_index: usize,
    end_index: usize,
) -> Option<TokenDelimiter> {
    let node_points = api.getNodePoints();
    let block_start_index = api.getBlockStartIndex();
    let block_end_index = api.getBlockEndIndex();

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

        if c != AsciiCodePoint::ASTERISK as i32 && c != AsciiCodePoint::UNDERSCORE as i32 {
            i += 1;
            continue;
        }

        let delimiter_start_index = i;
        i += 1;
        while i < end_index && node_points[i].code_point == c {
            i += 1;
        }
        let delimiter_end_index = i;

        let is_left_flanking = is_opener_delimiter(
            node_points,
            delimiter_start_index,
            delimiter_end_index,
            block_end_index,
            start_index,
            end_index,
        );
        let is_right_flanking = is_closer_delimiter(
            node_points,
            delimiter_start_index,
            delimiter_end_index,
            block_start_index,
            start_index,
            end_index,
        );

        let mut is_opener = is_left_flanking;
        let mut is_closer = is_right_flanking;

        if c == AsciiCodePoint::UNDERSCORE as i32 && is_left_flanking && is_right_flanking {
            if delimiter_start_index > start_index
                && !is_punctuation_character(node_points[delimiter_start_index - 1].code_point)
            {
                is_opener = false;
            }

            let next_is_punctuation = node_points
                .get(delimiter_end_index)
                .is_some_and(|point| is_punctuation_character(point.code_point));
            if !next_is_punctuation {
                is_closer = false;
            }
        }

        if !is_opener && !is_closer {
            continue;
        }

        let thickness = delimiter_end_index - delimiter_start_index;
        return Some(TokenDelimiter {
            delimiter_type: match (is_opener, is_closer) {
                (true, true) => DelimiterType::Both,
                (true, false) => DelimiterType::Opener,
                (false, true) => DelimiterType::Closer,
                (false, false) => unreachable!(),
            },
            start_index: delimiter_start_index,
            end_index: delimiter_end_index,
            thickness,
            original_thickness: thickness,
        });
    }

    None
}

pub(crate) fn is_delimiter_pair(
    api: &dyn MatchInlinePhaseApi,
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
) -> IsDelimiterPairResult {
    let node_points = api.getNodePoints();

    let Some(opener) = node_points.get(opener_delimiter.start_index) else {
        return IsDelimiterPairResult::NotPaired {
            opener: true,
            closer: true,
        };
    };
    let Some(closer) = node_points.get(closer_delimiter.start_index) else {
        return IsDelimiterPairResult::NotPaired {
            opener: true,
            closer: true,
        };
    };

    let is_same_marker = opener.code_point == closer.code_point;
    let violates_mod_three = (matches!(opener_delimiter.delimiter_type, DelimiterType::Both)
        || matches!(closer_delimiter.delimiter_type, DelimiterType::Both))
        && (opener_delimiter.original_thickness + closer_delimiter.original_thickness) % 3 == 0
        && opener_delimiter.original_thickness % 3 != 0;

    if !is_same_marker || violates_mod_three {
        return IsDelimiterPairResult::NotPaired {
            opener: true,
            closer: true,
        };
    }

    IsDelimiterPairResult::Paired
}

pub(crate) fn process_delimiter_pair(
    api: &dyn MatchInlinePhaseApi,
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
    internal_tokens: &[InlineToken],
) -> ProcessDelimiterPairResult {
    let thickness = if opener_delimiter.thickness > 1 && closer_delimiter.thickness > 1 {
        2
    } else {
        1
    };

    let resolved_children = api.resolveInternalTokens(
        internal_tokens,
        opener_delimiter.end_index,
        closer_delimiter.start_index,
    );

    let node_type = if thickness == 2 {
        STRONG_TYPE
    } else {
        EMPHASIS_TYPE
    };

    let token = InlineToken::new(
        "",
        node_type,
        (
            opener_delimiter.end_index - thickness,
            closer_delimiter.start_index + thickness,
        ),
    )
    .with_data(EmphasisTokenData {
        thickness,
        children: resolved_children,
    });

    let remain_opener_delimiter = if opener_delimiter.thickness > thickness {
        Some(TokenDelimiter {
            delimiter_type: opener_delimiter.delimiter_type,
            start_index: opener_delimiter.start_index,
            end_index: opener_delimiter.end_index - thickness,
            thickness: opener_delimiter.thickness - thickness,
            original_thickness: opener_delimiter.original_thickness,
        })
    } else {
        None
    };

    let remain_closer_delimiter = if closer_delimiter.thickness > thickness {
        Some(TokenDelimiter {
            delimiter_type: closer_delimiter.delimiter_type,
            start_index: closer_delimiter.start_index + thickness,
            end_index: closer_delimiter.end_index,
            thickness: closer_delimiter.thickness - thickness,
            original_thickness: closer_delimiter.original_thickness,
        })
    } else {
        None
    };

    ProcessDelimiterPairResult {
        tokens: vec![token],
        remainOpenerDelimiter: remain_opener_delimiter,
        remainCloserDelimiter: remain_closer_delimiter,
    }
}

fn is_opener_delimiter(
    node_points: &[NodePoint],
    delimiter_start_index: usize,
    delimiter_end_index: usize,
    block_end_index: usize,
    start_index: usize,
    end_index: usize,
) -> bool {
    if delimiter_end_index == block_end_index {
        return false;
    }
    if delimiter_end_index == end_index {
        return true;
    }

    let Some(next) = node_points.get(delimiter_end_index) else {
        return false;
    };
    if is_unicode_whitespace_character(next.code_point) {
        return false;
    }

    if !is_punctuation_character(next.code_point) {
        return true;
    }

    if delimiter_start_index <= start_index {
        return true;
    }

    let prev = node_points[delimiter_start_index - 1].code_point;
    is_unicode_whitespace_character(prev) || is_punctuation_character(prev)
}

fn is_closer_delimiter(
    node_points: &[NodePoint],
    delimiter_start_index: usize,
    delimiter_end_index: usize,
    block_start_index: usize,
    start_index: usize,
    end_index: usize,
) -> bool {
    if delimiter_start_index == block_start_index {
        return false;
    }
    if delimiter_start_index == start_index {
        return true;
    }

    let prev = node_points[delimiter_start_index - 1].code_point;
    if is_unicode_whitespace_character(prev) {
        return false;
    }

    if !is_punctuation_character(prev) {
        return true;
    }

    if delimiter_end_index >= end_index {
        return true;
    }

    let next = node_points[delimiter_end_index].code_point;
    is_unicode_whitespace_character(next) || is_punctuation_character(next)
}
