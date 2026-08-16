use yozora_ast::LINK_TYPE;
use yozora_character::{is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    DelimiterType, InlineToken, MatchInlinePhaseApi, ResultOfRequiredEater,
};

use crate::types::{
    AutolinkExtensionContentType, AutolinkExtensionDelimiter, AutolinkExtensionTokenData,
};
use crate::util::email::{
    eat_extend_email_address_from_local_part_end, eat_extend_email_local_part,
};
use crate::util::protocol::eat_extended_protocol_autolink;
use crate::util::uri::{eat_domain_segment, eat_extended_url, eat_www_domain};

fn is_w(code_point: i32) -> bool {
    code_point == AsciiCodePoint::LOWERCASE_W as i32
        || code_point == AsciiCodePoint::UPPERCASE_W as i32
}

fn has_www_prefix(node_points: &[NodePoint], start_index: usize, end_index: usize) -> bool {
    start_index + 3 < end_index
        && is_w(node_points[start_index].code_point)
        && is_w(node_points[start_index + 1].code_point)
        && is_w(node_points[start_index + 2].code_point)
        && node_points[start_index + 3].code_point == AsciiCodePoint::DOT as i32
}

pub(crate) fn find_delimiter_entry(
    node_points: &[NodePoint],
    block_start_index: usize,
    start_index: usize,
    end_index: usize,
) -> Option<AutolinkExtensionDelimiter> {
    let mut email_checked_until = start_index;
    let mut www_checked_until = start_index;

    let mut eat_email_candidate = |candidate_start_index: usize| -> ResultOfRequiredEater {
        if candidate_start_index < email_checked_until {
            return ResultOfRequiredEater {
                valid: false,
                next_index: email_checked_until,
            };
        }

        let local_part_end_index =
            eat_extend_email_local_part(node_points, candidate_start_index, end_index);
        email_checked_until = local_part_end_index;
        let result = eat_extend_email_address_from_local_part_end(
            node_points,
            candidate_start_index,
            local_part_end_index,
            end_index,
        );
        if result.valid {
            result
        } else {
            ResultOfRequiredEater {
                valid: false,
                next_index: local_part_end_index,
            }
        }
    };

    let mut i = start_index;
    while i < end_index {
        let mut j = i;
        let mut flag = false;
        let mut email_start_index = None;
        while j < end_index {
            if flag || j == block_start_index {
                let protocol = eat_extended_protocol_autolink(node_points, j, end_index);
                if protocol.recognized {
                    if protocol.valid {
                        return Some(create_delimiter(
                            j,
                            protocol.next_index,
                            AutolinkExtensionContentType::Uri,
                        ));
                    }
                    j = std::cmp::max(j, protocol.next_index.saturating_sub(1));
                    flag = false;
                    email_start_index = None;
                    j += 1;
                    continue;
                }
            }

            let c = node_points[j].code_point;
            if is_whitespace_character(c)
                || c == AsciiCodePoint::ASTERISK as i32
                || c == AsciiCodePoint::UNDERSCORE as i32
                || c == AsciiCodePoint::TILDE as i32
                || c == AsciiCodePoint::OPEN_PARENTHESIS as i32
            {
                flag = true;
                if c == AsciiCodePoint::UNDERSCORE as i32 {
                    email_start_index.get_or_insert(j);
                } else {
                    email_start_index = None;
                }
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

        if let Some(email_start_index) = email_start_index {
            let email_result = eat_email_candidate(email_start_index);
            if email_result.valid {
                return Some(create_delimiter(
                    email_start_index,
                    email_result.next_index,
                    AutolinkExtensionContentType::Email,
                ));
            }
        }
        i = j;

        let url_result = eat_extended_url(node_points, i, end_index);
        let mut next_index = std::cmp::min(end_index, url_result.next_index);
        let mut content_type = url_result
            .valid
            .then_some(AutolinkExtensionContentType::Uri);
        if url_result.valid {
            next_index = url_result.next_index;
        }

        let has_www = has_www_prefix(node_points, i, end_index);
        if content_type.is_none() && has_www {
            if i < www_checked_until {
                next_index = std::cmp::min(next_index, www_checked_until);
            } else {
                let eat_result = eat_www_domain(node_points, i, end_index);
                next_index = std::cmp::min(next_index, eat_result.next_index);
                if eat_result.valid {
                    content_type = Some(AutolinkExtensionContentType::UriWww);
                    next_index = eat_result.next_index;
                } else {
                    www_checked_until = eat_result.next_index;
                }
            }
        }

        if content_type.is_none() {
            let eat_result = eat_email_candidate(i);
            next_index = std::cmp::min(next_index, eat_result.next_index);
            if eat_result.valid {
                content_type = Some(AutolinkExtensionContentType::Email);
                next_index = eat_result.next_index;
            }
        }

        let Some(content_type) = content_type else {
            if !has_www {
                let segment = eat_domain_segment(node_points, i, next_index);
                next_index = std::cmp::min(next_index, segment.next_index);
            }
            i = std::cmp::max(i, next_index.saturating_sub(1));
            i += 1;
            continue;
        };

        if next_index <= end_index {
            return Some(create_delimiter(i, next_index, content_type));
        }
        i = next_index;
    }

    None
}

pub(crate) fn process_single_delimiter(
    api: &dyn MatchInlinePhaseApi,
    delimiter: &AutolinkExtensionDelimiter,
) -> Vec<InlineToken> {
    let children_tokens =
        api.resolve_fallback_tokens(&[], delimiter.start_index, delimiter.end_index);
    vec![
        InlineToken::new("", LINK_TYPE, (delimiter.start_index, delimiter.end_index))
            .with_children(children_tokens)
            .with_data(AutolinkExtensionTokenData {
                content_type: delimiter.content_type,
            }),
    ]
}

fn create_delimiter(
    start_index: usize,
    end_index: usize,
    content_type: AutolinkExtensionContentType,
) -> AutolinkExtensionDelimiter {
    AutolinkExtensionDelimiter {
        delimiter_type: DelimiterType::Full,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
        content_type,
    }
}
