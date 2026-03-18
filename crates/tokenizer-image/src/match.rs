use yozora_character::{calc_escaped_string_from_node_points, create_node_point_generator};
use yozora_core_tokenizer::NodeInterval;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageToken {
    Text(NodeInterval),
    Image {
        interval: NodeInterval,
        url: String,
        title: Option<String>,
        alt: String,
    },
}

pub(crate) fn match_image_tokens(input: &str) -> Option<Vec<ImageToken>> {
    if !input.contains("![") {
        return None;
    }

    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    let mut last_emit = 0usize;
    let mut matched = false;

    while let Some(offset) = input[cursor..].find("![") {
        let start = cursor + offset;

        let Some((label_end, alt_raw)) = parse_image_label(input, start + 1) else {
            cursor = start + 2;
            continue;
        };

        let open_paren = label_end + 1;
        if input.as_bytes().get(open_paren).copied() != Some(b'(') {
            cursor = start + 2;
            continue;
        }

        let Some((url, title, end)) = parse_inline_image_tail(input, open_paren) else {
            cursor = start + 2;
            continue;
        };

        if start > last_emit {
            tokens.push(ImageToken::Text(NodeInterval {
                start_index: last_emit,
                end_index: start,
            }));
        }

        tokens.push(ImageToken::Image {
            interval: NodeInterval {
                start_index: start,
                end_index: end,
            },
            url,
            title,
            alt: normalize_alt_text(alt_raw),
        });

        matched = true;
        cursor = end;
        last_emit = end;
    }

    if !matched {
        return None;
    }

    if last_emit < input.len() {
        tokens.push(ImageToken::Text(NodeInterval {
            start_index: last_emit,
            end_index: input.len(),
        }));
    }

    Some(tokens)
}

fn parse_image_label(input: &str, bracket_start: usize) -> Option<(usize, &str)> {
    if input.as_bytes().get(bracket_start).copied()? != b'[' {
        return None;
    }

    let bytes = input.as_bytes();
    let mut i = bracket_start + 1;
    let mut depth = 1usize;
    let mut code_ticks = 0usize;

    while i < bytes.len() {
        if code_ticks > 0 {
            if bytes[i] == b'`' {
                let run = count_repeat(bytes, i, b'`');
                if run >= code_ticks {
                    code_ticks = 0;
                }
                i += run;
                continue;
            }
            i += 1;
            continue;
        }

        match bytes[i] {
            b'\\' => i = (i + 2).min(bytes.len()),
            b'`' => {
                code_ticks = count_repeat(bytes, i, b'`');
                i += code_ticks;
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((i, &input[bracket_start + 1..i]));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }

    None
}

fn parse_inline_image_tail(
    input: &str,
    open_paren: usize,
) -> Option<(String, Option<String>, usize)> {
    if input.as_bytes().get(open_paren).copied()? != b'(' {
        return None;
    }

    let bytes = input.as_bytes();
    let len = bytes.len();

    let mut i = eat_optional_ascii_whitespace(bytes, open_paren + 1);
    if i >= len {
        return None;
    }

    if bytes[i] == b')' {
        return Some((String::new(), None, i + 1));
    }

    let (url, destination_end) = parse_link_destination(input, i)?;
    let title_start = eat_optional_ascii_whitespace(bytes, destination_end);
    let has_separating_whitespace = title_start > destination_end;

    let (title, title_end) = if bytes.get(title_start).copied() == Some(b')') {
        (None, title_start)
    } else {
        if !has_separating_whitespace {
            return None;
        }
        parse_link_title(input, title_start)?
    };

    let close_index = eat_optional_ascii_whitespace(bytes, title_end);
    if bytes.get(close_index).copied() != Some(b')') {
        return None;
    }

    i = close_index + 1;
    Some((url, title, i))
}

fn parse_link_destination(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return None;
    }

    if bytes[start] == b'<' {
        let mut i = start + 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i = (i + 2).min(bytes.len()),
                b'<' | b'\n' | b'\r' => return None,
                b'>' => {
                    let raw = &input[start + 1..i];
                    let decoded = decode_escaped_content(raw, false);
                    return Some((encode_uri_like(&decoded), i + 1));
                }
                _ => i += 1,
            }
        }
        return None;
    }

    let mut i = start;
    let mut open_parens = 0i32;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b.is_ascii_control() {
            break;
        }

        match b {
            b'\\' => i = (i + 2).min(bytes.len()),
            b'(' => {
                open_parens += 1;
                i += 1;
            }
            b')' => {
                if open_parens == 0 {
                    break;
                }
                open_parens -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }

    if i == start || open_parens != 0 {
        return None;
    }

    let raw = &input[start..i];
    let decoded = decode_escaped_content(raw, false);
    Some((encode_uri_like(&decoded), i))
}

fn parse_link_title(input: &str, start: usize) -> Option<(Option<String>, usize)> {
    let bytes = input.as_bytes();
    let open = *bytes.get(start)?;

    if open == b')' {
        return Some((None, start));
    }

    match open {
        b'"' | b'\'' => {
            let mut i = start + 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i = (i + 2).min(bytes.len()),
                    b if b == open => {
                        let raw = &input[start + 1..i];
                        if contains_blank_line(raw) {
                            return None;
                        }
                        let title = decode_escaped_content(raw, false);
                        return Some((Some(title), i + 1));
                    }
                    _ => i += 1,
                }
            }
            None
        }
        b'(' => {
            let mut i = start + 1;
            let mut open_parens = 1i32;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i = (i + 2).min(bytes.len()),
                    b'(' => {
                        open_parens += 1;
                        i += 1;
                    }
                    b')' => {
                        open_parens -= 1;
                        if open_parens == 0 {
                            let raw = &input[start + 1..i];
                            if contains_blank_line(raw) {
                                return None;
                            }
                            let title = decode_escaped_content(raw, false);
                            return Some((Some(title), i + 1));
                        }
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
            None
        }
        _ => None,
    }
}

fn contains_blank_line(raw: &str) -> bool {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let bytes = normalized.as_bytes();

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'\n' {
            i += 1;
            continue;
        }

        let mut j = i + 1;
        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
            j += 1;
        }

        if j < bytes.len() && bytes[j] == b'\n' {
            return true;
        }

        i = j;
    }

    false
}

fn eat_optional_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn count_repeat(bytes: &[u8], mut index: usize, target: u8) -> usize {
    let mut count = 0usize;
    while index < bytes.len() && bytes[index] == target {
        count += 1;
        index += 1;
    }
    count
}

fn decode_escaped_content(input: &str, trim: bool) -> String {
    let chunks = create_node_point_generator(input);
    let Some(points) = chunks.first() else {
        return String::new();
    };

    calc_escaped_string_from_node_points(points, 0, points.len(), trim)
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

fn normalize_alt_text(input: &str) -> String {
    let decoded = decode_escaped_content(input, false);
    let stripped = strip_link_like_syntax(&decoded);
    strip_format_markers(&stripped)
}

fn strip_link_like_syntax(input: &str) -> String {
    let mut out = String::new();
    let mut cursor = 0usize;

    while cursor < input.len() {
        let Some(rel) = input[cursor..].find('[') else {
            out.push_str(&input[cursor..]);
            break;
        };

        let start = cursor + rel;
        out.push_str(&input[cursor..start]);

        let is_image = start > 0 && input.as_bytes()[start - 1] == b'!';
        let bracket_start = start;

        let Some((label_end, label_raw)) = parse_image_label(input, bracket_start) else {
            out.push('[');
            cursor = start + 1;
            continue;
        };

        let open_paren = label_end + 1;
        if input.as_bytes().get(open_paren).copied() != Some(b'(') {
            out.push('[');
            cursor = start + 1;
            continue;
        }

        let Some((_, _, end)) = parse_inline_image_tail(input, open_paren) else {
            out.push('[');
            cursor = start + 1;
            continue;
        };

        let normalized_label = normalize_alt_text(label_raw);
        if is_image && out.ends_with('!') {
            out.pop();
        }

        if !is_image && contains_valid_inline_link(label_raw) {
            let destination = &input[open_paren + 1..end - 1];
            out.push('[');
            out.push_str(&normalized_label);
            out.push_str("](");
            out.push_str(destination.trim());
            out.push(')');
        } else {
            out.push_str(&normalized_label);
        }

        cursor = end;
    }

    out
}

fn contains_valid_inline_link(input: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(rel) = input[cursor..].find('[') {
        let start = cursor + rel;
        if is_escaped(input, start) || (start > 0 && input.as_bytes()[start - 1] == b'!') {
            cursor = start + 1;
            continue;
        }

        let Some((label_end, _)) = parse_image_label(input, start) else {
            cursor = start + 1;
            continue;
        };

        let open_paren = label_end + 1;
        if input.as_bytes().get(open_paren).copied() != Some(b'(') {
            cursor = start + 1;
            continue;
        }

        if parse_inline_image_tail(input, open_paren).is_some() {
            return true;
        }

        cursor = start + 1;
    }

    false
}

fn strip_format_markers(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }

        if matches!(ch, '*' | '_' | '`') {
            continue;
        }

        out.push(ch);
    }
    out
}

fn is_escaped(input: &str, byte_index: usize) -> bool {
    if byte_index == 0 {
        return false;
    }

    let bytes = input.as_bytes();
    let mut idx = byte_index;
    let mut slash_count = 0usize;

    while idx > 0 {
        idx -= 1;
        if bytes[idx] == b'\\' {
            slash_count += 1;
        } else {
            break;
        }
    }

    slash_count % 2 == 1
}
