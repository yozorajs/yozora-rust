use yozora_ast::{LINK_REFERENCE_TYPE, LINK_TYPE};
use yozora_character::{calc_string_from_node_points, fold_case, AsciiCodePoint, NodePoint};

use crate::types::token::InlineToken;

/// Encode link destination in a uri-safe form.
pub fn encode_link_destination(destination: &str) -> String {
    encode_uri_like(destination)
}

/// Normalize link label into a lookup identifier.
pub fn resolve_label_to_identifier(label: &str) -> String {
    let collapsed = label
        .split(is_link_label_whitespace)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    fold_case(&collapsed)
}

fn is_link_label_whitespace(character: char) -> bool {
    matches!(
        character,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' '
    )
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

pub fn contains_link_token(tokens: &[InlineToken], start_index: usize, end_index: usize) -> bool {
    if start_index >= end_index || tokens.is_empty() {
        return false;
    }
    let mut stack = vec![(tokens, 0usize)];
    while let Some((tokens, index)) = stack.last_mut() {
        if *index >= tokens.len() {
            stack.pop();
            continue;
        }
        let token = &tokens[*index];
        *index += 1;
        if token.end_index <= start_index || token.start_index >= end_index {
            continue;
        }
        if is_link_token(token) {
            return true;
        }
        if !token.children.is_empty() {
            stack.push((token.children.as_slice(), 0));
        }
    }
    false
}

pub fn check_balanced_brackets_status(
    start_index: usize,
    end_index: usize,
    internal_tokens: &[InlineToken],
    node_points: &[NodePoint],
) -> i8 {
    let mut index = start_index;
    let mut bracket_count = 0i32;
    let update = |index: &mut usize, bracket_count: &mut i32| match node_points[*index].code_point {
        code_point if code_point == AsciiCodePoint::BACKSLASH as i32 => *index += 1,
        code_point if code_point == AsciiCodePoint::OPEN_BRACKET as i32 => *bracket_count += 1,
        code_point if code_point == AsciiCodePoint::CLOSE_BRACKET as i32 => *bracket_count -= 1,
        _ => {}
    };
    for token in internal_tokens {
        if token.start_index < start_index {
            continue;
        }
        if token.end_index > end_index {
            break;
        }
        while index < token.start_index {
            update(&mut index, &mut bracket_count);
            if bracket_count < 0 {
                return -1;
            }
            index += 1;
        }
        index = token.end_index;
    }
    while index < end_index {
        update(&mut index, &mut bracket_count);
        if bracket_count < 0 {
            return -1;
        }
        index += 1;
    }
    i8::from(bracket_count > 0)
}

pub fn is_valid_link_text(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    internal_tokens: &[InlineToken],
) -> bool {
    start_index <= end_index
        && end_index <= node_points.len()
        && !contains_link_token(internal_tokens, start_index, end_index)
        && check_balanced_brackets_status(start_index, end_index, internal_tokens, node_points) == 0
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
                | b'%'
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

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::{FOOTNOTE_TYPE, INLINE_CODE_TYPE};
    use yozora_character::create_node_point_generator;

    #[test]
    fn encodes_link_destinations_like_the_reference() {
        let safe = "AZaz09-_.+!*'(),%#@?=;:/&$~";
        assert_eq!(encode_link_destination(safe), safe);

        for (source, expected) in [
            (" ", "%20"),
            ("\"", "%22"),
            ("\\", "%5C"),
            ("[", "%5B"),
            ("]", "%5D"),
            ("|", "%7C"),
            ("^", "%5E"),
            ("`", "%60"),
            ("\u{1}", "%01"),
            ("ä", "%C3%A4"),
            ("😀", "%F0%9F%98%80"),
        ] {
            assert_eq!(
                encode_link_destination(source),
                expected,
                "source={source:?}"
            );
        }
    }

    #[test]
    fn preserves_existing_percent_escapes_and_is_idempotent() {
        for source in ["%2F", "%252F", "%ZZ", "foo%2", "100%", "foo bar", "ä", "😀"] {
            let encoded = encode_link_destination(source);
            assert_eq!(
                encode_link_destination(&encoded),
                encoded,
                "source={source:?}"
            );
        }

        assert_eq!(
            encode_link_destination("https://example.com/%252Fadmin"),
            "https://example.com/%252Fadmin"
        );
    }

    #[test]
    fn validates_balanced_link_text_ranges() {
        for (source, expected) in [
            ("", true),
            ("foo", true),
            ("foo [bar]", true),
            (r"foo \[bar", true),
            (r"foo \]bar", true),
            ("foo [bar", false),
            ("foo ]bar", false),
        ] {
            let node_points = create_node_point_generator(source)
                .pop()
                .unwrap_or_default();
            assert_eq!(
                is_valid_link_text(&node_points, 0, node_points.len(), &[]),
                expected,
                "source={source:?}"
            );
        }

        let node_points = create_node_point_generator("[foo]")
            .pop()
            .expect("expected node points");
        assert!(is_valid_link_text(&node_points, 1, 4, &[]));
    }

    #[test]
    fn normalizes_only_reference_ascii_label_whitespace() {
        assert_eq!(resolve_label_to_identifier(" a\t\nb "), "a b");
        assert_eq!(resolve_label_to_identifier("a\u{00A0}b"), "a\u{00A0}b");
        assert_eq!(resolve_label_to_identifier("a\u{2003}b"), "a\u{2003}b");
    }

    #[test]
    fn ignores_brackets_inside_higher_priority_tokens() {
        let node_points = create_node_point_generator("foo `]`")
            .pop()
            .expect("expected node points");
        let tokens = vec![InlineToken::new("test", INLINE_CODE_TYPE, (4, 7))];

        assert!(is_valid_link_text(
            &node_points,
            0,
            node_points.len(),
            &tokens,
        ));
    }

    #[test]
    fn rejects_nested_links_only_when_they_overlap_the_range() {
        let node_points = create_node_point_generator("foo")
            .pop()
            .expect("expected node points");
        let nested_link = InlineToken::new("test", LINK_TYPE, (2, 3));
        let footnote =
            InlineToken::new("test", FOOTNOTE_TYPE, (0, 3)).with_children(vec![nested_link]);

        assert!(!is_valid_link_text(
            &node_points,
            0,
            node_points.len(),
            std::slice::from_ref(&footnote),
        ));
        assert!(is_valid_link_text(
            &node_points,
            0,
            2,
            std::slice::from_ref(&footnote),
        ));

        for node_type in [LINK_TYPE, LINK_REFERENCE_TYPE] {
            let token = InlineToken::new("test", node_type, (0, node_points.len()));
            assert!(!is_valid_link_text(
                &node_points,
                0,
                node_points.len(),
                &[token],
            ));
        }
    }
}
