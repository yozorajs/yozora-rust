use yozora_ast::LINK_TYPE;
use yozora_character::{
    is_alphanumeric, is_ascii_control_character, is_ascii_digit_character, is_ascii_letter,
    is_whitespace_character, AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::{
    InlineToken, MatchInlinePhaseApi, ResultOfRequiredEater, TokenDelimiter,
};

use crate::parse::{AutolinkContentType, AutolinkTokenData};

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
    pub content_type: AutolinkContentType,
}

pub(crate) fn find_delimiter_entry(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<DelimiterEntry> {
    let mut i = start_index;
    while i < end_index {
        if node_points[i].code_point != AsciiCodePoint::OPEN_ANGLE as i32 {
            i += 1;
            continue;
        }

        let mut next_index = end_index;
        let mut content_type: Option<AutolinkContentType> = None;
        if i + 1 < end_index {
            let uri_result = eat_absolute_uri(node_points, i + 1, end_index);
            next_index = next_index.min(uri_result.next_index);
            if uri_result.valid {
                content_type = Some(AutolinkContentType::Uri);
                next_index = uri_result.next_index;
            } else {
                let email_result = eat_email_address(node_points, i + 1, end_index);
                next_index = next_index.min(email_result.next_index);
                if email_result.valid {
                    content_type = Some(AutolinkContentType::Email);
                    next_index = email_result.next_index;
                }
            }
        }

        let Some(content_type) = content_type else {
            let skip_to = std::cmp::max(i + 1, next_index.saturating_sub(1));
            i = skip_to;
            continue;
        };

        if next_index < end_index
            && node_points[next_index].code_point == AsciiCodePoint::CLOSE_ANGLE as i32
        {
            return Some(DelimiterEntry {
                delimiter: TokenDelimiter {
                    delimiter_type: yozora_core_tokenizer::DelimiterType::Full,
                    start_index: i,
                    end_index: next_index + 1,
                    thickness: next_index + 1 - i,
                    original_thickness: next_index + 1 - i,
                },
                content_type,
            });
        }

        let skip_to = std::cmp::max(i + 1, next_index.saturating_sub(1));
        i = skip_to;
    }

    None
}

pub(crate) fn process_single_delimiter(
    _api: &dyn MatchInlinePhaseApi,
    delimiter: &TokenDelimiter,
    content_type: AutolinkContentType,
) -> Vec<InlineToken> {
    vec![
        InlineToken::new("", LINK_TYPE, (delimiter.start_index, delimiter.end_index))
            .with_data(AutolinkTokenData { content_type }),
    ]
}

pub fn eat_absolute_uri(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    let schema = eat_autolink_schema(node_points, start_index, end_index);
    let mut next_index = schema.next_index;

    if !schema.valid
        || next_index >= end_index
        || node_points[next_index].code_point != AsciiCodePoint::COLON as i32
    {
        return ResultOfRequiredEater {
            valid: false,
            next_index,
        };
    }

    next_index += 1;
    while next_index < end_index {
        let c = node_points[next_index].code_point;
        if is_whitespace_character(c)
            || is_ascii_control_character(c)
            || c == AsciiCodePoint::OPEN_ANGLE as i32
            || c == AsciiCodePoint::CLOSE_ANGLE as i32
        {
            break;
        }

        next_index += 1;
    }

    ResultOfRequiredEater {
        valid: true,
        next_index,
    }
}

pub fn eat_autolink_schema(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    if start_index >= end_index {
        return ResultOfRequiredEater {
            valid: false,
            next_index: start_index,
        };
    }

    let mut i = start_index;
    let c = node_points[i].code_point;
    if !is_ascii_letter(c) {
        return ResultOfRequiredEater {
            valid: false,
            next_index: i + 1,
        };
    }

    i += 1;
    while i < end_index {
        let d = node_points[i].code_point;
        if is_alphanumeric(d)
            || d == AsciiCodePoint::PLUS_SIGN as i32
            || d == AsciiCodePoint::DOT as i32
            || d == AsciiCodePoint::MINUS_SIGN as i32
        {
            i += 1;
            continue;
        }

        break;
    }

    let count = i - start_index;
    if !(2..=32).contains(&count) {
        return ResultOfRequiredEater {
            valid: false,
            next_index: i + 1,
        };
    }

    ResultOfRequiredEater {
        valid: true,
        next_index: i,
    }
}

pub fn eat_email_address(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    let mut i = start_index;

    while i < end_index {
        let c = node_points[i].code_point;
        if is_ascii_letter(c) || is_ascii_digit_character(c) {
            i += 1;
            continue;
        }

        if c != AsciiCodePoint::DOT as i32
            && c != AsciiCodePoint::EXCLAMATION_MARK as i32
            && c != AsciiCodePoint::NUMBER_SIGN as i32
            && c != AsciiCodePoint::DOLLAR_SIGN as i32
            && c != AsciiCodePoint::PERCENT_SIGN as i32
            && c != AsciiCodePoint::AMPERSAND as i32
            && c != AsciiCodePoint::SINGLE_QUOTE as i32
            && c != AsciiCodePoint::ASTERISK as i32
            && c != AsciiCodePoint::PLUS_SIGN as i32
            && c != AsciiCodePoint::SLASH as i32
            && c != AsciiCodePoint::EQUALS_SIGN as i32
            && c != AsciiCodePoint::QUESTION_MARK as i32
            && c != AsciiCodePoint::CARET as i32
            && c != AsciiCodePoint::UNDERSCORE as i32
            && c != AsciiCodePoint::BACKTICK as i32
            && c != AsciiCodePoint::OPEN_BRACE as i32
            && c != AsciiCodePoint::VERTICAL_SLASH as i32
            && c != AsciiCodePoint::CLOSE_BRACE as i32
            && c != AsciiCodePoint::TILDE as i32
            && c != AsciiCodePoint::MINUS_SIGN as i32
        {
            break;
        }

        i += 1;
    }

    if i == start_index
        || i + 1 >= end_index
        || node_points[i].code_point != AsciiCodePoint::AT_SIGN as i32
        || !is_alphanumeric(node_points[i + 1].code_point)
    {
        return ResultOfRequiredEater {
            valid: false,
            next_index: i + 1,
        };
    }

    i = eat_address_part0(node_points, i + 2, end_index);

    while i + 1 < end_index {
        let c = node_points[i].code_point;
        if c != AsciiCodePoint::DOT as i32 {
            break;
        }

        let d = node_points[i + 1].code_point;
        if !is_ascii_letter(d) && !is_ascii_digit_character(d) {
            break;
        }

        i = eat_address_part0(node_points, i + 2, end_index);
    }

    ResultOfRequiredEater {
        valid: true,
        next_index: i,
    }
}

fn eat_address_part0(node_points: &[NodePoint], start_index: usize, end_index: usize) -> usize {
    let mut i = start_index;
    let mut result: Option<usize> = None;

    let max_end_index = std::cmp::min(end_index, i + 62);
    while i < max_end_index {
        let c = node_points[i].code_point;
        if is_ascii_letter(c) || is_ascii_digit_character(c) {
            result = Some(i);
            i += 1;
            continue;
        }

        if c != AsciiCodePoint::MINUS_SIGN as i32 {
            break;
        }

        i += 1;
    }

    match result {
        Some(index) if index >= start_index => index + 1,
        _ => start_index,
    }
}
