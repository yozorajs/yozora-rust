use yozora_ast::{LINK_REFERENCE_TYPE, LINK_TYPE};
use yozora_character::{calc_string_from_node_points, fold_case, AsciiCodePoint, NodePoint};

use crate::types::token::InlineToken;

/// Encode link destination in a uri-safe form.
pub fn encode_link_destination(destination: &str) -> String {
    let mut decoded = destination.to_string();
    loop {
        let Ok(next) = try_percent_decode_once(&decoded) else {
            break;
        };

        if next == decoded {
            break;
        }

        decoded = next;
    }

    encode_uri_like(&decoded)
}

/// Normalize link label into a lookup identifier.
pub fn resolve_label_to_identifier(label: &str) -> String {
    let collapsed = label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    fold_case(&collapsed)
}

/// Resolve label text and normalized identifier from node-point interval.
pub fn resolve_link_label_and_identifier(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<(String, String)> {
    let label = calc_string_from_node_points(node_points, start_index, end_index, true);
    if label.is_empty() {
        return None;
    }

    let identifier = resolve_label_to_identifier(&label);
    Some((label, identifier))
}

/// Eat a `[label]` sequence where `node_points[start_index]` is `[`.
///
/// Returns `(next_index, label_and_identifier)`.
pub fn eat_link_label(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> (isize, Option<(String, String)>) {
    let mut i = start_index + 1;
    let max_end = usize::min(i + 1000, end_index);

    while i < max_end {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::BACKSLASH as i32 {
            i += 2;
            continue;
        }
        if c == AsciiCodePoint::OPEN_BRACKET as i32 {
            return (-1, None);
        }
        if c == AsciiCodePoint::CLOSE_BRACKET as i32 {
            return (
                (i + 1) as isize,
                resolve_link_label_and_identifier(node_points, start_index + 1, i),
            );
        }

        i += 1;
    }

    (-1, None)
}

pub fn is_link_token(token: &InlineToken) -> bool {
    token.node_type == LINK_TYPE || token.node_type == LINK_REFERENCE_TYPE
}

fn try_percent_decode_once(input: &str) -> Result<String, ()> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(());
            }

            let hi = from_hex(bytes[i + 1]).ok_or(())?;
            let lo = from_hex(bytes[i + 2]).ok_or(())?;
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).map_err(|_| ())
}

fn from_hex(ch: u8) -> Option<u8> {
    match ch {
        b'0'..=b'9' => Some(ch - b'0'),
        b'a'..=b'f' => Some(ch - b'a' + 10),
        b'A'..=b'F' => Some(ch - b'A' + 10),
        _ => None,
    }
}

fn encode_uri_like(input: &str) -> String {
    let mut out = String::new();
    for &b in input.as_bytes() {
        if is_allowed_uri_byte(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(hex_digit((b >> 4) & 0x0f));
            out.push(hex_digit(b & 0x0f));
        }
    }
    out
}

fn is_allowed_uri_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b';' | b','
                | b'/'
                | b'?'
                | b':'
                | b'@'
                | b'&'
                | b'='
                | b'+'
                | b'$'
                | b'-'
                | b'_'
                | b'.'
                | b'!'
                | b'~'
                | b'*'
                | b'\''
                | b'('
                | b')'
                | b'#'
        )
}

fn hex_digit(v: u8) -> char {
    match v {
        0..=9 => (b'0' + v) as char,
        10..=15 => (b'A' + (v - 10)) as char,
        _ => '0',
    }
}
