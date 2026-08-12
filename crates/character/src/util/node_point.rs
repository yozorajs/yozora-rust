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

impl IntoLiteralStrings for Vec<&str> {
    fn into_literal_strings(self) -> Vec<String> {
        self.into_iter().map(ToString::to_string).collect()
    }
}

impl IntoLiteralStrings for &[&str] {
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

    let mut pending_cr = false;
    let mut contents = Vec::new();
    for chunk in literal_strings.into_literal_strings() {
        let mut content = if pending_cr {
            pending_cr = false;
            format!("\r{chunk}")
        } else {
            chunk
        };

        if content.ends_with('\r') {
            content.pop();
            pending_cr = true;
        }
        contents.push(content);
    }
    if pending_cr {
        contents.push("\r".to_string());
    }

    for content in contents {
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
                            source_width: None,
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
                        source_width: None,
                    });
                    offset += 1;
                    column = 1;
                    line += 1;
                }
                x if x == AsciiCodePoint::CR as i32 => {
                    let is_crlf = i + 1 < code_points.len()
                        && code_points[i + 1] == AsciiCodePoint::LF as i32;
                    points.push(NodePoint {
                        line,
                        column,
                        offset,
                        code_point: VirtualCodePoint::LineEnd as i32,
                        source_width: is_crlf.then_some(2),
                    });
                    offset += if is_crlf { 2 } else { 1 };
                    column = 1;
                    line += 1;

                    if is_crlf {
                        i += 1;
                    }
                }
                x if x == AsciiCodePoint::NUL as i32 => {
                    points.push(NodePoint {
                        line,
                        column,
                        offset,
                        code_point: UnicodeCodePoint::ReplacementCharacter as i32,
                        source_width: None,
                    });
                    offset += 1;
                    column += 1;
                }
                _ => {
                    let width = char::from_u32(code_point as u32)
                        .map(char::len_utf16)
                        .unwrap_or(1);
                    points.push(NodePoint {
                        line,
                        column,
                        offset,
                        code_point,
                        source_width: None,
                    });
                    offset += width;
                    column += width;
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
            while j < end_index
                && node_points[j].code_point == VirtualCodePoint::Space as i32
                && node_points[j].offset == node_points[i].offset
            {
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
            while j < end_index
                && node_points[j].code_point == VirtualCodePoint::Space as i32
                && node_points[j].offset == node_points[i].offset
            {
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

#[cfg(test)]
mod tests {
    use super::create_node_point_generator;
    use crate::VirtualCodePoint;

    #[test]
    fn tracks_utf16_width_for_astral_characters() {
        let chunks = create_node_point_generator("a😀b");
        let points = &chunks[0];

        assert_eq!((points[1].column, points[1].offset), (2, 1));
        assert_eq!((points[2].column, points[2].offset), (4, 3));
    }

    #[test]
    fn preserves_crlf_width_across_chunks() {
        let chunks = create_node_point_generator(vec!["a\r", "\nb"]);
        let points = chunks.iter().flatten().copied().collect::<Vec<_>>();

        assert_eq!(points[1].code_point, VirtualCodePoint::LineEnd as i32);
        assert_eq!(points[1].source_width, Some(2));
        assert_eq!(
            (points[2].line, points[2].column, points[2].offset),
            (2, 1, 3)
        );
    }
}
