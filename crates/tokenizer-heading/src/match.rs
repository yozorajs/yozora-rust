#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadingToken {
    pub depth: u8,
    pub content: String,
}

pub(crate) fn match_heading_token(input: &str) -> Option<HeadingToken> {
    if input.contains('\n') {
        return None;
    }

    let leading_spaces = input.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return None;
    }

    let line = &input[leading_spaces..];
    if !line.starts_with('#') {
        return None;
    }

    let mut depth = 0usize;
    for ch in line.chars() {
        if ch == '#' {
            depth += 1;
        } else {
            break;
        }
    }

    if depth == 0 || depth > 6 {
        return None;
    }

    let after_opening = &line[depth..];
    if let Some(ch) = after_opening.chars().next() {
        if ch != ' ' && ch != '\t' {
            return None;
        }
    }

    let content = strip_closing_sequence(after_opening).trim().to_string();

    Some(HeadingToken {
        depth: depth as u8,
        content,
    })
}

fn strip_closing_sequence(after_opening: &str) -> &str {
    let trimmed_end = after_opening.trim_end_matches([' ', '\t']);
    let bytes = trimmed_end.as_bytes();

    let mut hash_start = bytes.len();
    while hash_start > 0 && bytes[hash_start - 1] == b'#' {
        hash_start -= 1;
    }

    if hash_start == bytes.len() {
        return trimmed_end;
    }

    if hash_start == 0 || (bytes[hash_start - 1] != b' ' && bytes[hash_start - 1] != b'\t') {
        return trimmed_end;
    }

    let mut content_end = hash_start;
    while content_end > 0 && (bytes[content_end - 1] == b' ' || bytes[content_end - 1] == b'\t') {
        content_end -= 1;
    }

    &trimmed_end[..content_end]
}
