use yozora_character::{is_ascii_upper_letter, AsciiCodePoint, NodePoint};

pub fn eat_start_condition4(node_points: &[NodePoint], i: usize, end: usize) -> Option<usize> {
    (i + 1 < end
        && node_points[i].code_point == AsciiCodePoint::EXCLAMATION_MARK as i32
        && is_ascii_upper_letter(node_points[i + 1].code_point))
    .then_some(i + 2)
}

pub fn eat_end_condition4(node_points: &[NodePoint], start: usize, end: usize) -> Option<usize> {
    (start..end).find_map(|i| {
        (node_points[i].code_point == AsciiCodePoint::CLOSE_ANGLE as i32).then_some(i + 1)
    })
}
