use yozora_ast::FOOTNOTE_TYPE;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    DelimiterType, InlineToken, IsDelimiterPairResult, MatchInlinePhaseApi,
    ProcessDelimiterPairResult,
};
use yozora_tokenizer_link::check_balanced_brackets_status;

use crate::parse::FootnoteTokenData;
use crate::types::FootnoteDelimiter;

pub(crate) fn find_delimiter_entry(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<FootnoteDelimiter> {
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
                    return Some(create_delimiter(DelimiterType::Opener, i, i + 2));
                }
            }
            x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
                return Some(create_delimiter(DelimiterType::Closer, i, i + 1));
            }
            _ => {}
        }

        i += 1;
    }

    None
}

pub(crate) fn is_delimiter_pair(
    api: &dyn MatchInlinePhaseApi,
    opener_delimiter: &FootnoteDelimiter,
    closer_delimiter: &FootnoteDelimiter,
    internal_tokens: &[InlineToken],
) -> IsDelimiterPairResult {
    if contains_footnote(
        internal_tokens,
        opener_delimiter.end_index,
        closer_delimiter.start_index,
    ) {
        return IsDelimiterPairResult::NotPaired {
            opener: false,
            closer: false,
        };
    }

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

fn contains_footnote(tokens: &[InlineToken], start_index: usize, end_index: usize) -> bool {
    let mut stack = vec![(tokens, 0usize)];

    while let Some((tokens, index)) = stack.last_mut() {
        if *index >= tokens.len() {
            stack.pop();
            continue;
        }

        let token = &tokens[*index];
        *index += 1;
        if token.start_index >= end_index || token.end_index <= start_index {
            continue;
        }
        if token.node_type == FOOTNOTE_TYPE {
            return true;
        }
        if !token.children.is_empty() {
            stack.push((token.children.as_slice(), 0));
        }
    }

    false
}

pub(crate) fn process_delimiter_pair(
    api: &dyn MatchInlinePhaseApi,
    opener_delimiter: &FootnoteDelimiter,
    closer_delimiter: &FootnoteDelimiter,
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
    .with_children(children_tokens)
    .with_data(FootnoteTokenData);

    ProcessDelimiterPairResult {
        tokens: vec![token],
        remain_opener_delimiter: None,
        remain_closer_delimiter: None,
    }
}

fn create_delimiter(
    delimiter_type: DelimiterType,
    start_index: usize,
    end_index: usize,
) -> FootnoteDelimiter {
    FootnoteDelimiter {
        delimiter_type,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
    }
}
