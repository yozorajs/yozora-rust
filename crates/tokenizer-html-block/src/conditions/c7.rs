use yozora_character::{is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::eat_optional_whitespaces;

use crate::util::eat_html_attribute;

pub fn eat_start_condition7(
    node_points: &[NodePoint],
    start: usize,
    end: usize,
    tag: &str,
    potential_open_tag: bool,
) -> Option<usize> {
    if matches!(tag, "pre" | "script" | "style") || start >= end {
        return None;
    }
    let mut i = start;
    if potential_open_tag {
        while let Some(result) = eat_html_attribute(node_points, i, end) {
            i = result.next_index;
        }
        i = eat_optional_whitespaces(node_points, i, end);
        if i >= end {
            return None;
        }
        if node_points[i].code_point == AsciiCodePoint::SLASH as i32 {
            i += 1;
        }
    } else {
        i = eat_optional_whitespaces(node_points, start, end);
    }
    if i >= end || node_points[i].code_point != AsciiCodePoint::CLOSE_ANGLE as i32 {
        return None;
    }
    ((i + 1..end).all(|j| is_whitespace_character(node_points[j].code_point))).then_some(end)
}
