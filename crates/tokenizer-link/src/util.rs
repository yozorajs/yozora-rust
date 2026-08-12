use yozora_character::{
    is_ascii_control_character, is_ascii_punctuation_character, is_whitespace_character,
    AsciiCodePoint, NodePoint, VirtualCodePoint,
};
use yozora_core_tokenizer::eat_optional_blank_lines;

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

pub fn eat_link_title(node_points: &[NodePoint], start_index: usize, end_index: usize) -> isize {
    if start_index >= end_index {
        return -1;
    }

    let mut i = start_index;
    let title_wrap_symbol = node_points[i].code_point;
    match title_wrap_symbol {
        code_point
            if code_point == AsciiCodePoint::DOUBLE_QUOTE as i32
                || code_point == AsciiCodePoint::SINGLE_QUOTE as i32 =>
        {
            i += 1;
            while i < end_index {
                match node_points[i].code_point {
                    code_point if code_point == AsciiCodePoint::BACKSLASH as i32 => {
                        i += 1;
                    }
                    code_point if code_point == title_wrap_symbol => {
                        return (i + 1) as isize;
                    }
                    code_point if code_point == VirtualCodePoint::LineEnd as i32 => {
                        let j = eat_optional_blank_lines(node_points, start_index, i);
                        if node_points[j].line > node_points[i].line + 1 {
                            return -1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        code_point if code_point == AsciiCodePoint::OPEN_PARENTHESIS as i32 => {
            let mut open_parens = 1i32;
            i += 1;
            while i < end_index {
                match node_points[i].code_point {
                    code_point if code_point == AsciiCodePoint::BACKSLASH as i32 => {
                        i += 1;
                    }
                    code_point if code_point == VirtualCodePoint::LineEnd as i32 => {
                        let j = eat_optional_blank_lines(node_points, start_index, i);
                        if node_points[j].line > node_points[i].line + 1 {
                            return -1;
                        }
                    }
                    code_point if code_point == AsciiCodePoint::OPEN_PARENTHESIS as i32 => {
                        open_parens += 1;
                    }
                    code_point if code_point == AsciiCodePoint::CLOSE_PARENTHESIS as i32 => {
                        open_parens -= 1;
                        if open_parens == 0 {
                            return (i + 1) as isize;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        code_point if code_point == AsciiCodePoint::CLOSE_PARENTHESIS as i32 => {
            return i as isize;
        }
        _ => return -1,
    }
    -1
}
