use yozora_character::{is_ascii_digit_character, is_ascii_letter, AsciiCodePoint, NodePoint};

pub fn eat_html_tag_name(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<usize> {
    if start_index >= end_index || !is_ascii_letter(node_points[start_index].code_point) {
        return None;
    }

    let mut i = start_index;
    while i < end_index {
        let code_point = node_points[i].code_point;
        if is_ascii_letter(code_point)
            || is_ascii_digit_character(code_point)
            || code_point == AsciiCodePoint::MINUS_SIGN as i32
        {
            i += 1;
            continue;
        }
        return Some(i);
    }
    Some(i)
}
