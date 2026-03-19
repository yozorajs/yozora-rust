use yozora_ast::FOOTNOTE_TYPE;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    DelimiterType, InlineToken, IsDelimiterPairResult, MatchInlinePhaseApi,
    ProcessDelimiterPairResult, TokenDelimiter,
};

use crate::parse::FootnoteTokenData;

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
}

pub(crate) fn find_delimiter_entry(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<DelimiterEntry> {
    let mut i = start_index;
    while i < end_index {
        let code_point = node_points[i].code_point;
        match code_point {
            x if x == AsciiCodePoint::BACKSLASH as i32 => {
                i = (i + 2).min(end_index);
                continue;
            }
            x if x == AsciiCodePoint::CARET as i32 => {
                if i + 1 < end_index
                    && node_points[i + 1].code_point == AsciiCodePoint::OPEN_BRACKET as i32
                {
                    return Some(DelimiterEntry {
                        delimiter: create_delimiter(DelimiterType::Opener, i, i + 2),
                    });
                }
            }
            x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
                return Some(DelimiterEntry {
                    delimiter: create_delimiter(DelimiterType::Closer, i, i + 1),
                });
            }
            _ => {}
        }

        i += 1;
    }

    None
}

pub(crate) fn is_delimiter_pair(
    api: &dyn MatchInlinePhaseApi,
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
    internal_tokens: &[InlineToken],
) -> IsDelimiterPairResult {
    let status = check_balanced_brackets_status(
        opener_delimiter.end_index,
        closer_delimiter.start_index,
        internal_tokens,
        api.get_node_points(),
    );

    match status {
        -1 => IsDelimiterPairResult::NotPaired {
            opener: false,
            closer: true,
        },
        0 => IsDelimiterPairResult::Paired,
        1 => IsDelimiterPairResult::NotPaired {
            opener: true,
            closer: false,
        },
        _ => IsDelimiterPairResult::NotPaired {
            opener: false,
            closer: false,
        },
    }
}

pub(crate) fn process_delimiter_pair(
    api: &dyn MatchInlinePhaseApi,
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
    internal_tokens: &[InlineToken],
) -> ProcessDelimiterPairResult {
    let children_tokens = api.resolve_internal_tokens(
        internal_tokens,
        opener_delimiter.end_index,
        closer_delimiter.start_index,
    );

    let token = InlineToken::new(
        "",
        FOOTNOTE_TYPE,
        (opener_delimiter.start_index, closer_delimiter.end_index),
    )
    .with_data(FootnoteTokenData { children_tokens });

    ProcessDelimiterPairResult {
        tokens: vec![token],
        remainOpenerDelimiter: None,
        remainCloserDelimiter: None,
    }
}

pub(crate) fn check_balanced_brackets_status(
    start_index: usize,
    end_index: usize,
    internal_tokens: &[InlineToken],
    node_points: &[NodePoint],
) -> i8 {
    let mut i = start_index;
    let mut bracket_count = 0i32;

    let update = |idx: usize, count: &mut i32, i_ref: &mut usize| match node_points[idx].code_point
    {
        x if x == AsciiCodePoint::BACKSLASH as i32 => {
            *i_ref += 1;
        }
        x if x == AsciiCodePoint::OPEN_BRACKET as i32 => {
            *count += 1;
        }
        x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
            *count -= 1;
        }
        _ => {}
    };

    for token in internal_tokens {
        if token.start_index < start_index {
            continue;
        }
        if token.end_index > end_index {
            break;
        }

        while i < token.start_index {
            update(i, &mut bracket_count, &mut i);
            if bracket_count < 0 {
                return -1;
            }
            i += 1;
        }

        i = token.end_index;
    }

    while i < end_index {
        update(i, &mut bracket_count, &mut i);
        if bracket_count < 0 {
            return -1;
        }
        i += 1;
    }

    if bracket_count > 0 {
        1
    } else {
        0
    }
}

fn create_delimiter(
    delimiter_type: DelimiterType,
    start_index: usize,
    end_index: usize,
) -> TokenDelimiter {
    TokenDelimiter {
        delimiter_type,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
    }
}
