use yozora_character::{is_alphanumeric, is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::ResultOfRequiredEater;
use yozora_tokenizer_autolink::eat_autolink_schema;

#[derive(Debug, Clone, Copy)]
pub struct DomainSegmentEatResult {
    pub valid: bool,
    pub next_index: usize,
    pub has_underscore: bool,
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
