use yozora_character::{
    is_alphanumeric, is_ascii_control_character, is_ascii_letter, is_whitespace_character,
    AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::ResultOfRequiredEater;

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
