use crate::constant::ascii::AsciiCodePoint;
use crate::constant::unicode::UnicodeCodePoint;
use crate::constant::virtual_code_point::VirtualCodePoint;
use crate::types::{CodePoint, NodePoint};
use crate::util::character::is_whitespace_character;
use crate::util::charset::ascii::is_ascii_punctuation_character;
use crate::util::entity_reference::eat_entity_reference;

pub trait IntoLiteralStrings {
    fn into_literal_strings(self) -> Vec<String>;
}

impl IntoLiteralStrings for &str {
    fn into_literal_strings(self) -> Vec<String> {
        vec![self.to_string()]
    }
}

impl IntoLiteralStrings for String {
    fn into_literal_strings(self) -> Vec<String> {
        vec![self]
    }
}

impl IntoLiteralStrings for Vec<String> {
    fn into_literal_strings(self) -> Vec<String> {
        self
    }
}

impl<'a> IntoLiteralStrings for Vec<&'a str> {
    fn into_literal_strings(self) -> Vec<String> {
        self.into_iter().map(ToString::to_string).collect()
    }
}

impl<'a> IntoLiteralStrings for &'a [&'a str] {
    fn into_literal_strings(self) -> Vec<String> {
        self.iter().map(ToString::to_string).collect()
    }
}

pub fn create_node_point_generator<T: IntoLiteralStrings>(
    literal_strings: T,
) -> Vec<Vec<NodePoint>> {
    let mut offset = 0usize;
    let mut column = 1usize;
    let mut line = 1usize;
    let mut chunks: Vec<Vec<NodePoint>> = Vec::new();

    for content in literal_strings.into_literal_strings() {
        let code_points: Vec<CodePoint> = content.chars().map(|c| c as i32).collect();
        let mut points: Vec<NodePoint> = Vec::with_capacity(code_points.len());

        let mut i = 0usize;
        while i < code_points.len() {
            let code_point = code_points[i];
            match code_point {
                x if x == AsciiCodePoint::HT as i32 => {
                    for _ in 0..4 {
                        points.push(NodePoint {
                            line,
                            column,
                            offset,
                            code_point: VirtualCodePoint::Space as i32,
                        });
                    }
                    offset += 1;
                    column += 1;
                }
                x if x == AsciiCodePoint::LF as i32 => {
                    points.push(NodePoint {
                        line,
                        column,
                        offset,
                        code_point: VirtualCodePoint::LineEnd as i32,
                    });
                    offset += 1;
                    column = 1;
                    line += 1;
                }
                x if x == AsciiCodePoint::CR as i32 => {
                    points.push(NodePoint {
                        line,
                        column,
                        offset,
                        code_point: VirtualCodePoint::LineEnd as i32,
                    });
                    offset += 1;
                    column = 1;
                    line += 1;

                    if i + 1 < code_points.len() && code_points[i + 1] == AsciiCodePoint::LF as i32
                    {
                        offset += 1;
                        i += 1;
                    }
                }
                x if x == AsciiCodePoint::NUL as i32 => {
                    points.push(NodePoint {
                        line,
                        column,
                        offset,
                        code_point: UnicodeCodePoint::ReplacementCharacter as i32,
                    });
                    offset += 1;
                    column += 1;
                }
                _ => {
                    points.push(NodePoint {
                        line,
                        column,
                        offset,
                        code_point,
                    });
                    offset += 1;
                    column += 1;
                }
            }
            i += 1;
        }

        chunks.push(points);
    }

    chunks
}

pub fn calc_trim_boundary_of_code_points(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> (usize, usize) {
    if start_index >= end_index {
        return (start_index, end_index);
    }

    let mut left = start_index;
    let mut right = end_index - 1;

    while left <= right && is_whitespace_character(node_points[left].code_point) {
        left += 1;
    }

    while left <= right && is_whitespace_character(node_points[right].code_point) {
        right = right.saturating_sub(1);
    }

    (left, right + 1)
}

pub fn calc_string_from_node_points(
    node_points: &[NodePoint],
    mut start_index: usize,
    mut end_index: usize,
    trim: bool,
) -> String {
    if trim {
        (start_index, end_index) =
            calc_trim_boundary_of_code_points(node_points, start_index, end_index);
    }

    let mut result = String::new();
    let mut i = start_index;
    while i < end_index {
        let c = node_points[i].code_point;
        if c == VirtualCodePoint::Space as i32 {
            let mut j = i + 1;
            while j < end_index && node_points[j].code_point == VirtualCodePoint::Space as i32 {
                j += 1;
            }

            let count = j - i;
            let tab_count = count >> 2;
            let space_count = count & 3;

            for _ in 0..space_count {
                result.push(' ');
            }
            for _ in 0..tab_count {
                result.push('\t');
            }
            i = j;
            continue;
        }

        if c == VirtualCodePoint::LineEnd as i32 {
            result.push('\n');
            i += 1;
            continue;
        }

        if let Some(ch) = char::from_u32(c as u32) {
            result.push(ch);
        }
        i += 1;
    }

    result
}

pub fn calc_escaped_string_from_node_points(
    node_points: &[NodePoint],
    mut start_index: usize,
    mut end_index: usize,
    trim: bool,
) -> String {
    if trim {
        (start_index, end_index) =
            calc_trim_boundary_of_code_points(node_points, start_index, end_index);
    }

    let mut result = String::new();
    let mut i = start_index;
    while i < end_index {
        let c = node_points[i].code_point;

        if c == AsciiCodePoint::BACKSLASH as i32 && i + 1 < end_index {
            let d = node_points[i + 1].code_point;
            if is_ascii_punctuation_character(d) {
                if let Some(ch) = char::from_u32(d as u32) {
                    result.push(ch);
                }
                i += 2;
                continue;
            }
            result.push('\\');
            i += 1;
            continue;
        }

        if c == VirtualCodePoint::Space as i32 {
            let mut j = i + 1;
            while j < end_index && node_points[j].code_point == VirtualCodePoint::Space as i32 {
                j += 1;
            }

            let count = j - i;
            let tab_count = count >> 2;
            let space_count = count & 3;
            for _ in 0..space_count {
                result.push(' ');
            }
            for _ in 0..tab_count {
                result.push('\t');
            }
            i = j;
            continue;
        }

        if c == VirtualCodePoint::LineEnd as i32 {
            result.push('\n');
            i += 1;
            continue;
        }

        if c == AsciiCodePoint::AMPERSAND as i32 {
            if let Some(entity) = eat_entity_reference(node_points, i + 1, end_index) {
                result.push_str(&entity.value);
                i = entity.next_index;
                continue;
            }
            result.push('&');
            i += 1;
            continue;
        }

        if let Some(ch) = char::from_u32(c as u32) {
            result.push(ch);
        }
        i += 1;
    }

    result
}
