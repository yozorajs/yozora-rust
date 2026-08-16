use yozora_character::{AsciiCodePoint, NodePoint};

pub fn eat_start_condition5(node_points: &[NodePoint], i: usize, end: usize) -> Option<usize> {
    (i + 7 < end
        && node_points[i].code_point == AsciiCodePoint::EXCLAMATION_MARK as i32
        && node_points[i + 1].code_point == AsciiCodePoint::OPEN_BRACKET as i32
        && node_points[i + 2].code_point == AsciiCodePoint::UPPERCASE_C as i32
        && node_points[i + 3].code_point == AsciiCodePoint::UPPERCASE_D as i32
        && node_points[i + 4].code_point == AsciiCodePoint::UPPERCASE_A as i32
        && node_points[i + 5].code_point == AsciiCodePoint::UPPERCASE_T as i32
        && node_points[i + 6].code_point == AsciiCodePoint::UPPERCASE_A as i32
        && node_points[i + 7].code_point == AsciiCodePoint::OPEN_BRACKET as i32)
        .then_some(i + 8)
}

pub fn eat_end_condition5(node_points: &[NodePoint], start: usize, end: usize) -> Option<usize> {
    (start..end).find_map(|i| {
        (i + 2 < end
            && node_points[i].code_point == AsciiCodePoint::CLOSE_BRACKET as i32
            && node_points[i + 1].code_point == AsciiCodePoint::CLOSE_BRACKET as i32
            && node_points[i + 2].code_point == AsciiCodePoint::CLOSE_ANGLE as i32)
            .then_some(i + 3)
    })
}
