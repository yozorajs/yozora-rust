use yozora_character::{calc_escaped_string_from_node_points, create_node_point_generator};
use yozora_core_tokenizer::NodeInterval;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinkToken {
    Text(NodeInterval),
    Link {
        interval: NodeInterval,
        url: String,
        title: Option<String>,
        label: String,
    },
}

pub(crate) fn match_link_tokens(input: &str) -> Option<Vec<LinkToken>> {
    if !input.contains('[') {
        return None;
    }

    let image_label_ranges = collect_image_label_ranges(input);
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    let mut last_emit = 0usize;
    let mut matched = false;

    while let Some(offset) = input[cursor..].find('[') {
        let start = cursor + offset;
        if start > 0 && input.as_bytes()[start - 1] == b'!' && !is_escaped(input, start - 1) {
            cursor = start + 1;
            continue;
        }
        if is_in_ranges(start, &image_label_ranges) {
            cursor = start + 1;
            continue;
        }
        if is_escaped(input, start) {
            cursor = start + 1;
            continue;
        }

        let Some((label_end, label_raw)) = parse_link_text(input, start) else {
            cursor = start + 1;
            continue;
        };

        let open_paren = label_end + 1;
        if input.as_bytes().get(open_paren).copied() != Some(b'(') {
            cursor = start + 1;
            continue;
        }

        if contains_valid_inline_link(label_raw) {
            // Links may not contain other links at any level.
            cursor = start + 1;
            continue;
        }

        let Some((url, title, end)) = parse_inline_link_tail(input, open_paren) else {
            cursor = start + 1;
            continue;
        };

        if start > last_emit {
            tokens.push(LinkToken::Text(NodeInterval {
                start_index: last_emit,
                end_index: start,
            }));
        }

        tokens.push(LinkToken::Link {
            interval: NodeInterval {
                start_index: start,
                end_index: end,
            },
            url,
            title,
            label: label_raw.to_string(),
        });

        matched = true;
        cursor = end;
        last_emit = end;
    }

    if !matched {
        return None;
    }

    if last_emit < input.len() {
        tokens.push(LinkToken::Text(NodeInterval {
            start_index: last_emit,
            end_index: input.len(),
        }));
    }

    Some(tokens)
}

fn collect_image_label_ranges(input: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let bytes = input.as_bytes();
    let mut cursor = 0usize;

    while let Some(offset) = input[cursor..].find("![") {
        let bang = cursor + offset;
        if is_escaped(input, bang) {
            cursor = bang + 1;
            continue;
        }

        let bracket_start = bang + 1;
        let Some((label_end, _)) = parse_link_text(input, bracket_start) else {
            cursor = bang + 2;
            continue;
        };

        let open_paren = label_end + 1;
        if bytes.get(open_paren).copied() != Some(b'(')
            || parse_inline_link_tail(input, open_paren).is_none()
        {
            cursor = bang + 2;
            continue;
        }

        if label_end > bracket_start {
            ranges.push((bracket_start + 1, label_end));
        }
        cursor = label_end + 1;
    }

    ranges
}

fn is_in_ranges(index: usize, ranges: &[(usize, usize)]) -> bool {
    ranges
        .iter()
        .any(|(start, end)| index >= *start && index < *end)
}

fn parse_link_text(input: &str, start: usize) -> Option<(usize, &str)> {
    if input.as_bytes().get(start).copied()? != b'[' {
        return None;
    }

    let bytes = input.as_bytes();
    let mut i = start + 1;
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
            b'<' => {
                if let Some(next) = skip_angle_segment(bytes, i) {
                    i = next;
                } else {
                    i += 1;
                }
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((i, &input[start + 1..i]));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }

    None
}

fn parse_inline_link_tail(
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
                    return Some((encode_link_destination(&decoded), i + 1));
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
    Some((encode_link_destination(&decoded), i))
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

fn contains_valid_inline_link(input: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(offset) = input[cursor..].find('[') {
        let start = cursor + offset;
        if start > 0 && input.as_bytes()[start - 1] == b'!' {
            cursor = start + 1;
            continue;
        }
        if is_escaped(input, start) {
            cursor = start + 1;
            continue;
        }

        let Some((label_end, _label_raw)) = parse_link_text(input, start) else {
            cursor = start + 1;
            continue;
        };

        let open_paren = label_end + 1;
        if input.as_bytes().get(open_paren).copied() != Some(b'(') {
            cursor = start + 1;
            continue;
        }

        if parse_inline_link_tail(input, open_paren).is_some() {
            return true;
        }

        cursor = start + 1;
    }

    false
}

fn eat_optional_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
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

fn count_repeat(bytes: &[u8], mut index: usize, target: u8) -> usize {
    let mut count = 0usize;
    while index < bytes.len() && bytes[index] == target {
        count += 1;
        index += 1;
    }
    count
}

fn skip_angle_segment(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start).copied()? != b'<' {
        return None;
    }

    let mut i = start + 1;
    let mut quote: Option<u8> = None;

    while i < bytes.len() {
        if let Some(q) = quote {
            match bytes[i] {
                b'\\' => i = (i + 2).min(bytes.len()),
                b if b == q => {
                    quote = None;
                    i += 1;
                }
                _ => i += 1,
            }
            continue;
        }

        match bytes[i] {
            b'"' | b'\'' => {
                quote = Some(bytes[i]);
                i += 1;
            }
            b'>' => return Some(i + 1),
            b'\n' | b'\r' => return None,
            _ => i += 1,
        }
    }

    None
}

fn decode_escaped_content(input: &str, trim: bool) -> String {
    let chunks = create_node_point_generator(input);
    let Some(points) = chunks.first() else {
        return String::new();
    };

    calc_escaped_string_from_node_points(points, 0, points.len(), trim)
}

fn encode_link_destination(destination: &str) -> String {
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
