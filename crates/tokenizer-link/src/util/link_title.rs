use yozora_character::{AsciiCodePoint, NodePoint, VirtualCodePoint};
use yozora_core_tokenizer::eat_optional_blank_lines;

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
