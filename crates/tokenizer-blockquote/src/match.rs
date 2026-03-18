use yozora_core_tokenizer::{
    leading_indent_columns, split_indent_prefix, strip_indent_columns_with_base,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlockquoteToken {
    pub consumed_lines: usize,
    pub value: String,
}

pub(crate) fn match_blockquote_token(lines: &[&str]) -> Option<BlockquoteToken> {
    if lines.is_empty() {
        return None;
    }

    let mut body_lines = Vec::new();
    let mut consumed_lines = 0usize;
    let mut previous_was_lazy = false;
    let mut previous_line_blank = false;
    let mut previous_was_code_line = false;

    for line in lines {
        if let Some(content) = strip_blockquote_marker(line) {
            previous_line_blank = content.trim().is_empty();
            previous_was_code_line =
                is_fenced_code_marker_line(&content) || starts_with_indented_code(&content);
            body_lines.push(content);
            consumed_lines += 1;
            previous_was_lazy = false;
            continue;
        }

        if consumed_lines > 0 && !line.trim().is_empty() {
            // Lazy continuation for paragraph text in blockquote.
            if previous_line_blank || previous_was_code_line || starts_block_construct(line) {
                break;
            }

            if is_setext_underline(line) && !previous_was_lazy {
                break;
            }

            if is_setext_underline(line) {
                body_lines.push(format!("\\{}", line));
            } else {
                body_lines.push((*line).to_string());
            }
            consumed_lines += 1;
            previous_was_lazy = true;
            previous_line_blank = line.trim().is_empty();
            previous_was_code_line = false;
            continue;
        }

        break;
    }

    if consumed_lines == 0 {
        return None;
    }

    Some(BlockquoteToken {
        consumed_lines,
        value: body_lines.join("\n"),
    })
}

fn starts_with_indented_code(line: &str) -> bool {
    leading_indent_columns(line) >= 4
}

fn strip_blockquote_marker(line: &str) -> Option<String> {
    let (leading_indent, leading_bytes) = split_indent_prefix(line, 0);
    if leading_indent >= 4 {
        return None;
    }

    let remainder = &line[leading_bytes..];
    if !remainder.starts_with('>') {
        return None;
    }

    let content = &remainder[1..];
    if content.starts_with(' ') || content.starts_with('\t') {
        return strip_indent_columns_with_base(content, 1, leading_indent + 1);
    }
    Some(content.to_string())
}

fn is_setext_underline(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let Some(marker) = trimmed.chars().next() else {
        return false;
    };
    if marker != '=' && marker != '-' {
        return false;
    }

    trimmed.chars().all(|ch| ch == marker)
}

fn is_fenced_code_marker_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn starts_block_construct(line: &str) -> bool {
    if leading_indent_columns(line) >= 4 {
        return false;
    }

    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.starts_with('#') || is_fenced_code_marker_line(trimmed) {
        return true;
    }

    if is_thematic_break_line(trimmed) {
        return true;
    }

    if starts_list_marker(trimmed) {
        return true;
    }

    false
}

fn is_thematic_break_line(trimmed: &str) -> bool {
    let mut marker: Option<char> = None;
    let mut count = 0usize;

    for ch in trimmed.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }

        if !matches!(ch, '-' | '*' | '_') {
            return false;
        }

        if let Some(m) = marker {
            if m != ch {
                return false;
            }
        } else {
            marker = Some(ch);
        }

        count += 1;
    }

    marker.is_some() && count >= 3
}

fn starts_list_marker(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    if matches!(bytes[0], b'-' | b'+' | b'*') {
        return bytes.len() == 1 || bytes[1] == b' ' || bytes[1] == b'\t';
    }

    let mut idx = 0usize;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }

    if idx == 0 || idx > 9 || idx >= bytes.len() {
        return false;
    }

    if bytes[idx] != b'.' && bytes[idx] != b')' {
        return false;
    }

    idx += 1;
    idx >= bytes.len() || bytes[idx] == b' ' || bytes[idx] == b'\t'
}
