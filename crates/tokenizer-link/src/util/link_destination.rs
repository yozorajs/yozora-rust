use yozora_character::{
    is_ascii_control_character, is_ascii_punctuation_character, is_whitespace_character,
    AsciiCodePoint, NodePoint, VirtualCodePoint,
};

const MAX_LINK_DESTINATION_PAREN_DEPTH: i32 = 32;

pub fn eat_link_destination(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> isize {
    if start_index >= end_index {
        return -1;
    }

    let mut i = start_index;
    if node_points[i].code_point == AsciiCodePoint::OPEN_ANGLE as i32 {
        i += 1;
        while i < end_index {
            match node_points[i].code_point {
                code_point if code_point == AsciiCodePoint::BACKSLASH as i32 => {
                    if i + 1 < end_index
                        && is_ascii_punctuation_character(node_points[i + 1].code_point)
                    {
                        i += 1;
                    }
                }
                code_point
                    if code_point == AsciiCodePoint::OPEN_ANGLE as i32
                        || code_point == VirtualCodePoint::LineEnd as i32 =>
                {
                    return -1;
                }
                code_point if code_point == AsciiCodePoint::CLOSE_ANGLE as i32 => {
                    return (i + 1) as isize;
                }
                _ => {}
            }
            i += 1;
        }
        return -1;
    }

    let mut open_parens_count = 0i32;
    while i < end_index {
        let code_point = node_points[i].code_point;
        match code_point {
            code_point if code_point == AsciiCodePoint::BACKSLASH as i32 => {
                if i + 1 < end_index
                    && is_ascii_punctuation_character(node_points[i + 1].code_point)
                {
                    i += 1;
                }
            }
            code_point if code_point == AsciiCodePoint::OPEN_PARENTHESIS as i32 => {
                open_parens_count += 1;
                if open_parens_count > MAX_LINK_DESTINATION_PAREN_DEPTH {
                    return -1;
                }
            }
            code_point if code_point == AsciiCodePoint::CLOSE_PARENTHESIS as i32 => {
                open_parens_count -= 1;
                if open_parens_count < 0 {
                    return i as isize;
                }
            }
            _ if is_whitespace_character(code_point) || is_ascii_control_character(code_point) => {
                return i as isize;
            }
            _ => {}
        }
        i += 1;
    }

    if open_parens_count == 0 {
        i as isize
    } else {
        -1
    }
}
