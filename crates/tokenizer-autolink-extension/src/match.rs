use yozora_ast::LINK_TYPE;
use yozora_character::{is_alphanumeric, is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    DelimiterType, InlineToken, MatchInlinePhaseApi, ResultOfRequiredEater, TokenDelimiter,
};
use yozora_tokenizer_autolink::eat_autolink_schema;

use crate::parse::{AutolinkExtensionContentType, AutolinkExtensionTokenData};

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
    pub content_type: AutolinkExtensionContentType,
}

#[derive(Debug, Clone, Copy)]
pub struct DomainSegmentEatResult {
    pub valid: bool,
    pub next_index: usize,
    pub has_underscore: bool,
}

#[derive(Debug, Clone, Copy)]
struct ExtendedProtocolEatResult {
    recognized: bool,
    valid: bool,
    next_index: usize,
}

pub(crate) fn find_delimiter_entry(
    node_points: &[NodePoint],
    block_start_index: usize,
    start_index: usize,
    end_index: usize,
) -> Option<DelimiterEntry> {
    let mut i = start_index;
    while i < end_index {
        // Extended autolink only starts at line beginning or after a delimiter boundary.
        let mut j = i;
        let mut flag = false;
        while j < end_index {
            let c = node_points[j].code_point;
            if is_whitespace_character(c)
                || c == AsciiCodePoint::ASTERISK as i32
                || c == AsciiCodePoint::UNDERSCORE as i32
                || c == AsciiCodePoint::TILDE as i32
                || c == AsciiCodePoint::OPEN_PARENTHESIS as i32
            {
                flag = true;
                j += 1;
                continue;
            }

            if flag || j == block_start_index {
                break;
            }

            flag = false;
            j += 1;
        }

        if j >= end_index {
            break;
        }
        i = j;

        let protocol_result = eat_extended_protocol_autolink(node_points, i, end_index);
        if protocol_result.recognized {
            if protocol_result.valid {
                return Some(DelimiterEntry {
                    delimiter: TokenDelimiter {
                        delimiter_type: DelimiterType::Full,
                        start_index: i,
                        end_index: protocol_result.next_index,
                        thickness: protocol_result.next_index - i,
                        original_thickness: protocol_result.next_index - i,
                    },
                    content_type: AutolinkExtensionContentType::Uri,
                });
            }
            i = protocol_result.next_index.max(i + 1);
            continue;
        }

        let mut next_index = end_index;
        let mut content_type: Option<AutolinkExtensionContentType> = None;

        let url_result = eat_extended_url(node_points, i, end_index);
        next_index = next_index.min(url_result.next_index);
        if url_result.valid {
            content_type = Some(AutolinkExtensionContentType::Uri);
            next_index = url_result.next_index;
        } else {
            let www_result = eat_www_domain(node_points, i, end_index);
            next_index = next_index.min(www_result.next_index);
            if www_result.valid {
                content_type = Some(AutolinkExtensionContentType::UriWww);
                next_index = www_result.next_index;
            } else {
                let email_result = eat_extend_email_address(node_points, i, end_index);
                next_index = next_index.min(email_result.next_index);
                if email_result.valid {
                    content_type = Some(AutolinkExtensionContentType::Email);
                    next_index = email_result.next_index;
                }
            }
        }

        let Some(content_type) = content_type else {
            i = std::cmp::max(i + 1, next_index);
            continue;
        };

        if next_index <= end_index {
            return Some(DelimiterEntry {
                delimiter: TokenDelimiter {
                    delimiter_type: DelimiterType::Full,
                    start_index: i,
                    end_index: next_index,
                    thickness: next_index.saturating_sub(i),
                    original_thickness: next_index.saturating_sub(i),
                },
                content_type,
            });
        }

        i = std::cmp::max(i + 1, next_index);
    }

    None
}

pub(crate) fn process_single_delimiter(
    api: &dyn MatchInlinePhaseApi,
    delimiter: &TokenDelimiter,
    content_type: AutolinkExtensionContentType,
) -> Vec<InlineToken> {
    let children_tokens =
        api.resolve_fallback_tokens(&[], delimiter.start_index, delimiter.end_index);
    vec![
        InlineToken::new("", LINK_TYPE, (delimiter.start_index, delimiter.end_index))
            .with_children(children_tokens.clone())
            .with_data(AutolinkExtensionTokenData { content_type }),
    ]
}

pub fn eat_extended_url(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    let schema = eat_autolink_schema(node_points, start_index, end_index);
    let next_index = schema.next_index;

    if !schema.valid
        || next_index + 3 >= end_index
        || node_points[next_index].code_point != AsciiCodePoint::COLON as i32
        || node_points[next_index + 1].code_point != AsciiCodePoint::SLASH as i32
        || node_points[next_index + 2].code_point != AsciiCodePoint::SLASH as i32
    {
        return ResultOfRequiredEater {
            valid: false,
            next_index: next_index + 1,
        };
    }

    let mut result = eat_valid_domain(node_points, next_index + 3, end_index);
    if result.valid {
        result.next_index = eat_optional_domain_follows(node_points, result.next_index, end_index);
    }
    result
}

pub fn eat_www_domain(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    let segment = eat_domain_segment(node_points, start_index, end_index);
    let next_index = segment.next_index;

    if !segment.valid
        || next_index >= end_index
        || node_points[next_index].code_point != AsciiCodePoint::DOT as i32
        || next_index.saturating_sub(start_index) != 3
    {
        return ResultOfRequiredEater {
            valid: false,
            next_index,
        };
    }

    for point in &node_points[start_index..next_index] {
        let c = point.code_point;
        if c != AsciiCodePoint::LOWERCASE_W as i32 && c != AsciiCodePoint::UPPERCASE_W as i32 {
            return ResultOfRequiredEater {
                valid: false,
                next_index,
            };
        }
    }

    let mut result = eat_valid_domain(node_points, next_index + 1, end_index);
    if result.valid {
        result.next_index = eat_optional_domain_follows(node_points, result.next_index, end_index);
    }
    result
}

pub fn eat_optional_domain_follows(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> usize {
    let mut next_index = start_index;
    while next_index < end_index {
        let c = node_points[next_index].code_point;
        if is_whitespace_character(c) || c == AsciiCodePoint::OPEN_ANGLE as i32 {
            break;
        }
        next_index += 1;
    }

    while next_index > start_index {
        let c = node_points[next_index - 1].code_point;
        if c == AsciiCodePoint::QUESTION_MARK as i32
            || c == AsciiCodePoint::EXCLAMATION_MARK as i32
            || c == AsciiCodePoint::DOT as i32
            || c == AsciiCodePoint::COMMA as i32
            || c == AsciiCodePoint::COLON as i32
            || c == AsciiCodePoint::ASTERISK as i32
            || c == AsciiCodePoint::UNDERSCORE as i32
            || c == AsciiCodePoint::TILDE as i32
        {
            next_index -= 1;
            continue;
        }
        break;
    }

    if next_index > start_index
        && node_points[next_index - 1].code_point == AsciiCodePoint::CLOSE_PARENTHESIS as i32
    {
        let mut parenthesis_balance = 0isize;
        for point in &node_points[start_index..next_index] {
            let c = point.code_point;
            if c == AsciiCodePoint::OPEN_PARENTHESIS as i32 {
                parenthesis_balance += 1;
            } else if c == AsciiCodePoint::CLOSE_PARENTHESIS as i32 {
                parenthesis_balance -= 1;
            }
        }

        while parenthesis_balance < 0
            && next_index > start_index
            && node_points[next_index - 1].code_point == AsciiCodePoint::CLOSE_PARENTHESIS as i32
        {
            parenthesis_balance += 1;
            next_index -= 1;
        }
    }

    if next_index > start_index
        && node_points[next_index - 1].code_point == AsciiCodePoint::SEMICOLON as i32
    {
        let mut i = next_index - 1;
        while i > start_index {
            let c = node_points[i - 1].code_point;
            if !is_alphanumeric(c) {
                break;
            }
            i -= 1;
        }

        if i > start_index && node_points[i - 1].code_point == AsciiCodePoint::AMPERSAND as i32 {
            next_index = i - 1;
        }
    }

    next_index
}

pub fn eat_valid_domain(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    let segment = eat_domain_segment(node_points, start_index, end_index);
    if !segment.valid || segment.next_index >= end_index {
        return ResultOfRequiredEater {
            valid: false,
            next_index: segment.next_index,
        };
    }

    let mut next_index = segment.next_index;
    let mut count_of_period = 0usize;
    let mut count_of_underscore_of_last_two_segment = if segment.has_underscore {
        2usize
    } else {
        0usize
    };

    while next_index < end_index {
        if node_points[next_index].code_point != AsciiCodePoint::DOT as i32 {
            break;
        }

        let segment = eat_domain_segment(node_points, next_index + 1, end_index);
        if !segment.valid {
            break;
        }

        next_index = segment.next_index;
        count_of_period += 1;
        count_of_underscore_of_last_two_segment >>= 1;
        if segment.has_underscore {
            count_of_underscore_of_last_two_segment |= 2;
        }
    }

    if count_of_period == 0 || count_of_underscore_of_last_two_segment != 0 {
        return ResultOfRequiredEater {
            valid: false,
            next_index,
        };
    }

    ResultOfRequiredEater {
        valid: true,
        next_index,
    }
}

pub fn eat_domain_segment(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> DomainSegmentEatResult {
    let mut i = start_index;
    let mut has_underscore = false;
    while i < end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::UNDERSCORE as i32 {
            has_underscore = true;
            i += 1;
            continue;
        }

        if !is_alphanumeric(c) && c != AsciiCodePoint::MINUS_SIGN as i32 {
            break;
        }

        i += 1;
    }

    if i > start_index {
        DomainSegmentEatResult {
            valid: true,
            next_index: i,
            has_underscore,
        }
    } else {
        DomainSegmentEatResult {
            valid: false,
            next_index: i,
            has_underscore,
        }
    }
}

pub fn eat_extend_email_address(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    let local_part_end_index = eat_extend_email_local_part(node_points, start_index, end_index);
    eat_extend_email_address_from_local_part_end(
        node_points,
        start_index,
        local_part_end_index,
        end_index,
    )
}

pub fn eat_extend_email_address_from_local_part_end(
    node_points: &[NodePoint],
    start_index: usize,
    local_part_end_index: usize,
    end_index: usize,
) -> ResultOfRequiredEater {
    let mut i = local_part_end_index;
    if i == start_index
        || i + 2 >= end_index
        || node_points[i].code_point != AsciiCodePoint::AT_SIGN as i32
        || !is_alphanumeric(node_points[i + 1].code_point)
    {
        return ResultOfRequiredEater {
            valid: false,
            next_index: i + 1,
        };
    }

    let mut count_of_period = 0usize;
    i += 2;
    while i < end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::DOT as i32 {
            if node_points[i - 1].code_point == AsciiCodePoint::DOT as i32 {
                break;
            }
            count_of_period += 1;
            i += 1;
            continue;
        }
        if is_alphanumeric(c)
            || c == AsciiCodePoint::MINUS_SIGN as i32
            || c == AsciiCodePoint::UNDERSCORE as i32
        {
            i += 1;
            continue;
        }
        break;
    }

    let last_character = node_points[i - 1].code_point;
    if last_character == AsciiCodePoint::MINUS_SIGN as i32
        || last_character == AsciiCodePoint::UNDERSCORE as i32
    {
        return ResultOfRequiredEater {
            valid: false,
            next_index: i,
        };
    }

    if last_character == AsciiCodePoint::DOT as i32 {
        i -= 1;
        count_of_period = count_of_period.saturating_sub(1);
    }

    if count_of_period == 0 {
        return ResultOfRequiredEater {
            valid: false,
            next_index: i,
        };
    }

    ResultOfRequiredEater {
        valid: true,
        next_index: i,
    }
}

pub fn eat_extend_email_local_part(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> usize {
    let mut i = start_index;
    while i < end_index {
        let c = node_points[i].code_point;
        if is_alphanumeric(c)
            || c == AsciiCodePoint::DOT as i32
            || c == AsciiCodePoint::MINUS_SIGN as i32
            || c == AsciiCodePoint::UNDERSCORE as i32
            || c == AsciiCodePoint::PLUS_SIGN as i32
        {
            i += 1;
            continue;
        }
        break;
    }
    i
}

fn eat_extended_protocol_autolink(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ExtendedProtocolEatResult {
    let (prefix, allows_resource) = if has_prefix(node_points, start_index, end_index, "mailto:") {
        ("mailto:", false)
    } else if has_prefix(node_points, start_index, end_index, "xmpp:") {
        ("xmpp:", true)
    } else {
        return ExtendedProtocolEatResult {
            recognized: false,
            valid: false,
            next_index: start_index + 1,
        };
    };

    let address_start_index = start_index + prefix.len();
    let email = eat_extend_email_address(node_points, address_start_index, end_index);
    if !email.valid {
        return ExtendedProtocolEatResult {
            recognized: true,
            valid: false,
            next_index: email.next_index.max(address_start_index).min(end_index),
        };
    }

    let mut next_index = email.next_index;
    if allows_resource
        && next_index < end_index
        && node_points[next_index].code_point == AsciiCodePoint::SLASH as i32
    {
        let mut resource_end_index = next_index + 1;
        while resource_end_index < end_index {
            let code_point = node_points[resource_end_index].code_point;
            if !is_alphanumeric(code_point)
                && code_point != AsciiCodePoint::AT_SIGN as i32
                && code_point != AsciiCodePoint::DOT as i32
            {
                break;
            }
            resource_end_index += 1;
        }
        if resource_end_index > next_index + 1 {
            next_index = resource_end_index;
        }
    }

    ExtendedProtocolEatResult {
        recognized: true,
        valid: true,
        next_index,
    }
}

fn has_prefix(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    prefix: &str,
) -> bool {
    start_index + prefix.len() <= end_index
        && prefix
            .bytes()
            .enumerate()
            .all(|(index, byte)| node_points[start_index + index].code_point == i32::from(byte))
}
