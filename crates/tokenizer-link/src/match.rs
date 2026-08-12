use yozora_ast::LINK_TYPE;
use yozora_character::{
    calc_escaped_string_from_node_points, create_node_point_generator, AsciiCodePoint, NodePoint,
    VirtualCodePoint,
};
use yozora_core_tokenizer::{encode_link_destination, DelimiterType, InlineToken, TokenDelimiter};

use crate::parse::LinkTokenData;

#[derive(Debug, Clone)]
pub(crate) struct LinkDelimiterData {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
    pub data: Option<LinkDelimiterData>,
}

pub(crate) fn build_source(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> String {
    let mut source = String::new();
    for point in node_points
        .iter()
        .skip(start_index)
        .take(end_index.saturating_sub(start_index))
    {
        let code_point = point.code_point;
        let ch = if code_point == VirtualCodePoint::Space as i32 {
            Some(' ')
        } else if code_point == VirtualCodePoint::LineEnd as i32 {
            Some('\n')
        } else {
            char::from_u32(code_point as u32)
        };

        if let Some(ch) = ch {
            source.push(ch);
        }
    }
    source
}

pub(crate) fn build_char_starts(source: &str) -> Vec<usize> {
    source.char_indices().map(|(i, _)| i).collect()
}

pub(crate) fn find_link_delimiter_entry(
    source: &str,
    char_starts: &[usize],
    node_points: &[NodePoint],
    block_start_index: usize,
    block_end_index: usize,
    start_index: usize,
    end_index: usize,
) -> Option<DelimiterEntry> {
    let mut i = start_index;

    while i < end_index {
        let code_point = node_points[i].code_point;

        if code_point == AsciiCodePoint::BACKSLASH as i32 {
            i = (i + 2).min(end_index);
            continue;
        }

        if code_point == AsciiCodePoint::OPEN_BRACKET as i32 {
            // `![...]` belongs to image syntax; link tokenizer should skip it.
            if i > block_start_index
                && node_points[i - 1].code_point == AsciiCodePoint::EXCLAMATION_MARK as i32
                && !is_escaped_node_points(node_points, i - 1, block_start_index)
            {
                i += 1;
                continue;
            }

            return Some(DelimiterEntry {
                delimiter: create_delimiter(DelimiterType::Opener, i, i + 1),
                data: None,
            });
        }

        if code_point == AsciiCodePoint::CLOSE_BRACKET as i32
            && i + 1 < end_index
            && node_points[i + 1].code_point == AsciiCodePoint::OPEN_PARENTHESIS as i32
        {
            let local_open_paren = (i + 1).saturating_sub(block_start_index);
            let open_paren_byte = char_to_byte_index(char_starts, source.len(), local_open_paren);

            let Some((url, title, end_byte)) = parse_inline_link_tail(source, open_paren_byte)
            else {
                i += 1;
                continue;
            };

            let local_end = byte_to_char_index(char_starts, source.len(), end_byte);
            let absolute_end = block_start_index + local_end;
            if absolute_end > block_end_index {
                i += 1;
                continue;
            }

            return Some(DelimiterEntry {
                delimiter: create_delimiter(DelimiterType::Closer, i, absolute_end),
                data: Some(LinkDelimiterData { url, title }),
            });
        }

        i += 1;
    }

    None
}

pub(crate) fn create_link_token(
    node_points: &[NodePoint],
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
    url: String,
    title: Option<String>,
    children_tokens: Vec<InlineToken>,
) -> InlineToken {
    let label = calc_escaped_string_from_node_points(
        node_points,
        opener_delimiter.end_index,
        closer_delimiter.start_index,
        false,
    );

    let token_children = children_tokens.clone();
    InlineToken::new(
        "",
        LINK_TYPE,
        (opener_delimiter.start_index, closer_delimiter.end_index),
    )
    .with_children(token_children)
    .with_data(LinkTokenData { url, title, label })
}

pub(crate) fn check_balanced_brackets_status(
    start_index: usize,
    end_index: usize,
    internal_tokens: &[InlineToken],
    node_points: &[NodePoint],
) -> i8 {
    let mut i = start_index;
    let mut bracket_count = 0i32;

    let update = |idx: usize, count: &mut i32, i_ref: &mut usize| match node_points[idx].code_point
    {
        x if x == AsciiCodePoint::BACKSLASH as i32 => {
            *i_ref += 1;
        }
        x if x == AsciiCodePoint::OPEN_BRACKET as i32 => {
            *count += 1;
        }
        x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
            *count -= 1;
        }
        _ => {}
    };

    for token in internal_tokens {
        if token.start_index < start_index {
            continue;
        }
        if token.end_index > end_index {
            break;
        }

        while i < token.start_index {
            update(i, &mut bracket_count, &mut i);
            if bracket_count < 0 {
                return -1;
            }
            i += 1;
        }

        i = token.end_index;
    }

    while i < end_index {
        update(i, &mut bracket_count, &mut i);
        if bracket_count < 0 {
            return -1;
        }
        i += 1;
    }

    if bracket_count > 0 {
        1
    } else {
        0
    }
}

pub(crate) fn is_escaped_node_points(
    node_points: &[NodePoint],
    index: usize,
    min_index: usize,
) -> bool {
    if index == 0 || index <= min_index {
        return false;
    }

    let mut i = index;
    let mut slash_count = 0usize;
    while i > min_index {
        i -= 1;
        if node_points[i].code_point == AsciiCodePoint::BACKSLASH as i32 {
            slash_count += 1;
            continue;
        }
        break;
    }

    slash_count % 2 == 1
}

fn create_delimiter(
    delimiter_type: DelimiterType,
    start_index: usize,
    end_index: usize,
) -> TokenDelimiter {
    TokenDelimiter {
        delimiter_type,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
    }
}

fn char_to_byte_index(char_starts: &[usize], source_len: usize, char_index: usize) -> usize {
    if char_index >= char_starts.len() {
        source_len
    } else {
        char_starts[char_index]
    }
}

fn byte_to_char_index(char_starts: &[usize], source_len: usize, byte_index: usize) -> usize {
    if byte_index >= source_len {
        return char_starts.len();
    }

    match char_starts.binary_search(&byte_index) {
        Ok(i) | Err(i) => i,
    }
}

pub(crate) fn parse_inline_link_tail(
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

fn eat_optional_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn decode_escaped_content(input: &str, trim: bool) -> String {
    let chunks = create_node_point_generator(input);
    let Some(points) = chunks.first() else {
        return String::new();
    };

    calc_escaped_string_from_node_points(points, 0, points.len(), trim)
}
