use yozora_character::{
    calc_escaped_string_from_node_points, create_node_point_generator, fold_case,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefinitionBlockToken {
    pub identifier: String,
    pub label: String,
    pub url: String,
    pub title: Option<String>,
    pub consumed_lines: usize,
}

pub(crate) fn match_definition(lines: &[&str]) -> Option<DefinitionBlockToken> {
    let first = *lines.first()?;
    if count_leading_spaces(first) >= 4 {
        return None;
    }

    let (label, mut consumed_lines, mut tail) = parse_definition_head(lines)?;
    let mut cursor_line_index = consumed_lines - 1;

    if tail.trim().is_empty() {
        let next_line = *lines.get(cursor_line_index + 1)?;
        if next_line.trim().is_empty() {
            return None;
        }

        tail = next_line.trim_start().to_string();
        consumed_lines += 1;
        cursor_line_index += 1;
    }

    let (url, rest_after_destination) = parse_destination(&tail)?;

    let mut title: Option<String> = None;
    let rest_trimmed = rest_after_destination.trim();

    if !rest_trimmed.is_empty() {
        if !starts_with_whitespace(rest_after_destination) {
            return None;
        }

        let title_start = rest_after_destination.trim_start();
        let following = if cursor_line_index + 1 < lines.len() {
            &lines[cursor_line_index + 1..]
        } else {
            &[][..]
        };
        let (parsed_title, consumed_following) =
            parse_title_with_following(title_start, following)?;
        title = Some(parsed_title);
        consumed_lines += consumed_following;
    } else if cursor_line_index + 1 < lines.len() {
        let next = lines[cursor_line_index + 1];
        let next_trimmed = next.trim_start();
        if starts_title(next_trimmed) {
            let following = if cursor_line_index + 2 < lines.len() {
                &lines[cursor_line_index + 2..]
            } else {
                &[][..]
            };

            if let Some((parsed_title, consumed_following)) =
                parse_title_with_following(next_trimmed, following)
            {
                title = Some(parsed_title);
                consumed_lines += 1 + consumed_following;
            }
        }
    }

    Some(DefinitionBlockToken {
        identifier: normalize_identifier(&label),
        label,
        url,
        title,
        consumed_lines,
    })
}

fn count_leading_spaces(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

fn starts_with_whitespace(input: &str) -> bool {
    input.chars().next().is_some_and(|ch| ch.is_whitespace())
}

fn parse_definition_head(lines: &[&str]) -> Option<(String, usize, String)> {
    let first = lines.first()?.trim_start();
    if !first.starts_with('[') {
        return None;
    }

    let mut label = String::new();
    let mut has_non_whitespace = false;

    for line_idx in 0..lines.len() {
        let line = if line_idx == 0 {
            first
        } else {
            lines[line_idx].trim_start()
        };

        if line_idx > 0 {
            if line.trim().is_empty() {
                return None;
            }
            label.push('\n');
        }

        let start = if line_idx == 0 { 1 } else { 0 };
        let mut chars = line[start..].char_indices().peekable();

        while let Some((rel, ch)) = chars.next() {
            let abs = start + rel;
            match ch {
                '\\' => {
                    let Some((_, next_ch)) = chars.next() else {
                        return None;
                    };
                    label.push('\\');
                    label.push(next_ch);
                    has_non_whitespace = true;
                }
                '[' => return None,
                ']' => {
                    let after = abs + 1;
                    if !line[after..].starts_with(':') || !has_non_whitespace {
                        return None;
                    }
                    return Some((label, line_idx + 1, line[after + 1..].to_string()));
                }
                _ => {
                    if !ch.is_whitespace() {
                        has_non_whitespace = true;
                    }
                    label.push(ch);
                }
            }
        }
    }

    None
}

fn parse_destination(input: &str) -> Option<(String, &str)> {
    let src = input.trim_start();
    if src.is_empty() {
        return None;
    }

    if let Some(rest) = src.strip_prefix('<') {
        let mut destination = String::new();
        let mut chars = rest.char_indices().peekable();

        while let Some((idx, ch)) = chars.next() {
            match ch {
                '\\' => {
                    let Some((_, next_ch)) = chars.next() else {
                        return None;
                    };
                    push_escaped_char(&mut destination, next_ch);
                }
                '<' | '\n' => return None,
                '>' => {
                    let consumed = 1 + idx + ch.len_utf8();
                    let decoded = decode_escaped_content(&destination);
                    return Some((encode_link_destination(&decoded), &src[consumed..]));
                }
                _ => destination.push(ch),
            }
        }

        return None;
    }

    let mut destination = String::new();
    let mut depth = 0i32;
    let mut end_byte = 0usize;
    let mut chars = src.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if ch.is_whitespace() || ch.is_control() {
            break;
        }

        match ch {
            '\\' => {
                if let Some((next_idx, next_ch)) = chars.next() {
                    push_escaped_char(&mut destination, next_ch);
                    end_byte = next_idx + next_ch.len_utf8();
                } else {
                    destination.push('\\');
                    end_byte = idx + ch.len_utf8();
                }
            }
            '(' => {
                depth += 1;
                destination.push(ch);
                end_byte = idx + ch.len_utf8();
            }
            ')' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                destination.push(ch);
                end_byte = idx + ch.len_utf8();
            }
            _ => {
                destination.push(ch);
                end_byte = idx + ch.len_utf8();
            }
        }
    }

    if destination.is_empty() || end_byte == 0 {
        return None;
    }

    let decoded = decode_escaped_content(&destination);
    Some((encode_link_destination(&decoded), &src[end_byte..]))
}

fn starts_title(input: &str) -> bool {
    input
        .chars()
        .next()
        .is_some_and(|ch| ch == '"' || ch == '\'' || ch == '(')
}

fn parse_title_with_following(start: &str, following: &[&str]) -> Option<(String, usize)> {
    let mut chars = start.chars();
    let opener = chars.next()?;
    if opener != '"' && opener != '\'' && opener != '(' {
        return None;
    }

    let mut value = String::new();
    let mut current_line = start;
    let mut line_index = 0usize;
    let mut start_offset = opener.len_utf8();

    loop {
        if line_index > 0 && current_line.trim().is_empty() {
            return None;
        }

        let mut escaped = false;
        let segment = &current_line[start_offset..];
        let mut consumed_on_line = None;

        for (idx, ch) in segment.char_indices() {
            if escaped {
                push_escaped_char(&mut value, ch);
                escaped = false;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                continue;
            }

            if opener == '(' {
                if ch == '(' {
                    return None;
                }
                if ch == ')' {
                    consumed_on_line = Some(idx + ch.len_utf8());
                    break;
                }
            } else if ch == opener {
                consumed_on_line = Some(idx + ch.len_utf8());
                break;
            }

            value.push(ch);
        }

        if escaped {
            value.push('\\');
        }

        if let Some(consumed) = consumed_on_line {
            let trailing = &segment[consumed..];
            if !trailing.trim().is_empty() {
                return None;
            }
            return Some((decode_escaped_content(&value), line_index));
        }

        let Some(next) = following.get(line_index).copied() else {
            return None;
        };

        value.push('\n');
        current_line = next;
        line_index += 1;
        start_offset = 0;
    }
}

fn decode_escaped_content(input: &str) -> String {
    let chunks = create_node_point_generator(input);
    let Some(points) = chunks.first() else {
        return String::new();
    };

    calc_escaped_string_from_node_points(points, 0, points.len(), false)
}

fn push_escaped_char(out: &mut String, ch: char) {
    if is_escapable_char(ch) {
        out.push(ch);
    } else {
        out.push('\\');
        out.push(ch);
    }
}

fn is_escapable_char(ch: char) -> bool {
    ch.is_ascii_punctuation()
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

fn normalize_identifier(label: &str) -> String {
    let mut out = String::new();
    for (idx, chunk) in label.split_whitespace().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(chunk);
    }
    fold_case(&out)
}
