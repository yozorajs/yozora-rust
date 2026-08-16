use yozora_character::{AsciiCodePoint, NodePoint};

pub fn eat_start_condition3(node_points: &[NodePoint], i: usize, end: usize) -> Option<usize> {
    (i < end && node_points[i].code_point == AsciiCodePoint::QUESTION_MARK as i32).then_some(i + 1)
}

pub fn eat_end_condition3(node_points: &[NodePoint], start: usize, end: usize) -> Option<usize> {
    (start..end).find_map(|i| {
        (i + 1 < end
            && node_points[i].code_point == AsciiCodePoint::QUESTION_MARK as i32
            && node_points[i + 1].code_point == AsciiCodePoint::CLOSE_ANGLE as i32)
            .then_some(i + 2)
    })
}
