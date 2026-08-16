use yozora_character::{
    calc_string_from_node_points, is_whitespace_character, AsciiCodePoint, NodePoint,
};

use crate::util::eat_html_tag_name;

const INCLUDED_TAGS: [&str; 3] = ["pre", "script", "style"];

pub fn eat_start_condition1(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    tag_name: &str,
) -> Option<usize> {
    if !INCLUDED_TAGS.contains(&tag_name) {
        return None;
    }
    if start_index >= end_index {
        return Some(end_index);
    }
    let c = node_points[start_index].code_point;
    (is_whitespace_character(c) || c == AsciiCodePoint::CLOSE_ANGLE as i32)
        .then_some(start_index + 1)
}

pub fn eat_end_condition1(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<usize> {
    let mut i = start_index;
    while i < end_index {
        if node_points[i].code_point == AsciiCodePoint::OPEN_ANGLE as i32
            && i + 3 < end_index
            && node_points[i + 1].code_point == AsciiCodePoint::SLASH as i32
        {
            let tag_name_start_index = i + 2;
            let Some(tag_name_end_index) =
                eat_html_tag_name(node_points, tag_name_start_index, end_index)
            else {
                i += 2;
                continue;
            };
            if tag_name_end_index >= end_index
                || node_points[tag_name_end_index].code_point != AsciiCodePoint::CLOSE_ANGLE as i32
            {
                i += 2;
                continue;
            }
            let tag_name = calc_string_from_node_points(
                node_points,
                tag_name_start_index,
                tag_name_end_index,
                false,
            )
            .to_lowercase();
            if INCLUDED_TAGS.contains(&tag_name.as_str()) {
                return Some(tag_name_end_index);
            }
        }
        i += 1;
    }
    None
}
