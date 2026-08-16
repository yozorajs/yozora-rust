use yozora_character::{
    is_alphanumeric, is_ascii_digit_character, is_ascii_letter, AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::ResultOfRequiredEater;

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
