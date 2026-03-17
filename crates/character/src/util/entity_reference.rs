use crate::constant::ascii::AsciiCodePoint;
use crate::constant::unicode::UnicodeCodePoint;
use crate::types::{CodePoint, NodePoint};
use crate::util::charset::ascii::is_ascii_digit_character;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityReference {
    pub next_index: usize,
    pub value: String,
}

fn resolve_named_entity(name: &str) -> Option<&'static str> {
    match name {
        "AMP;" | "amp;" => Some("&"),
        "LT;" | "lt;" => Some("<"),
        "GT;" | "gt;" => Some(">"),
        "QUOT;" | "quot;" => Some("\""),
        "APOS;" | "apos;" => Some("'"),
        "copy;" => Some("\u{00a9}"),
        "AElig;" => Some("\u{00c6}"),
        "Dcaron;" => Some("\u{010e}"),
        "frac34;" => Some("\u{00be}"),
        "HilbertSpace;" => Some("\u{210b}"),
        "DifferentialD;" => Some("\u{2146}"),
        "ClockwiseContourIntegral;" => Some("\u{2232}"),
        "ngE;" => Some("\u{2267}\u{0338}"),
        "ouml;" => Some("\u{00f6}"),
        "Auml;" | "auml;" => Some("\u{00e4}"),
        "nbsp;" => Some("\u{00a0}"),
        _ => None,
    }
}

pub fn eat_entity_reference(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<EntityReference> {
    if start_index + 1 >= end_index {
        return None;
    }

    let first = node_points[start_index].code_point;

    if first == AsciiCodePoint::NUMBER_SIGN as i32 {
        return parse_numeric_entity(node_points, start_index, end_index);
    }

    let mut raw = String::new();
    for (idx, point) in node_points
        .iter()
        .enumerate()
        .take(end_index)
        .skip(start_index)
    {
        let ch = char::from_u32(point.code_point as u32)?;
        raw.push(ch);
        if ch == ';' {
            if let Some(decoded) = resolve_named_entity(&raw) {
                return Some(EntityReference {
                    next_index: idx + 1,
                    value: decoded.to_string(),
                });
            }
            return None;
        }
        if raw.len() > 64 {
            return None;
        }
    }

    None
}

fn parse_numeric_entity(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<EntityReference> {
    let mut i = start_index + 1;
    let mut val: CodePoint = 0;
    let mut radix = 10;

    if i < end_index {
        let x = node_points[i].code_point;
        if x == AsciiCodePoint::LOWERCASE_X as i32 || x == AsciiCodePoint::UPPERCASE_X as i32 {
            radix = 16;
            i += 1;
        }
    }

    let mut read_count = 0usize;
    let max_digits = if radix == 16 { 6 } else { 7 };
    while i < end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::SEMICOLON as i32 {
            if read_count == 0 {
                return Some(EntityReference {
                    next_index: i + 1,
                    value: "\u{FFFD}".to_string(),
                });
            }

            let normalized = if val == 0
                || val > 0x10FFFF
                || (0xD800..=0xDFFF).contains(&val)
            {
                UnicodeCodePoint::ReplacementCharacter as i32
            } else {
                val
            };
            let value = char::from_u32(normalized as u32)
                .unwrap_or('\u{FFFD}')
                .to_string();
            return Some(EntityReference {
                next_index: i + 1,
                value,
            });
        }

        let digit = if radix == 16 {
            match c {
                x if x >= AsciiCodePoint::DIGIT0 as i32 && x <= AsciiCodePoint::DIGIT9 as i32 => {
                    x - AsciiCodePoint::DIGIT0 as i32
                }
                x if x >= AsciiCodePoint::UPPERCASE_A as i32
                    && x <= AsciiCodePoint::UPPERCASE_F as i32 =>
                {
                    x - AsciiCodePoint::UPPERCASE_A as i32 + 10
                }
                x if x >= AsciiCodePoint::LOWERCASE_A as i32
                    && x <= AsciiCodePoint::LOWERCASE_F as i32 =>
                {
                    x - AsciiCodePoint::LOWERCASE_A as i32 + 10
                }
                _ => return None,
            }
        } else if is_ascii_digit_character(c) {
            c - AsciiCodePoint::DIGIT0 as i32
        } else {
            return None;
        };

        read_count += 1;
        if read_count > max_digits {
            return None;
        }
        val = val
            .checked_mul(radix)
            .and_then(|v| v.checked_add(digit))
            .unwrap_or(UnicodeCodePoint::ReplacementCharacter as i32);
        i += 1;
    }

    None
}
