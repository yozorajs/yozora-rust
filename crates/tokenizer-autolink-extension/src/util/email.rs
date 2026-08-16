use yozora_character::{is_alphanumeric, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::ResultOfRequiredEater;

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

pub(crate) fn eat_extend_email_address_from_local_part_end(
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

pub(crate) fn eat_extend_email_local_part(
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
