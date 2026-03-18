use yozora_core_tokenizer::{leading_indent_columns, strip_indent_columns};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndentedCodeToken {
    pub consumed_lines: usize,
    pub value: String,
}

pub(crate) fn match_indented_code_token(lines: &[&str]) -> Option<IndentedCodeToken> {
    let first = *lines.first()?;
    if !is_indented_line(first) {
        return None;
    }

    let mut raw_lines = Vec::new();
    let mut consumed_lines = 0usize;

    for line in lines {
        if let Some(stripped) = strip_indented_prefix(line) {
            raw_lines.push(stripped);
            consumed_lines += 1;
            continue;
        }

        if line.trim().is_empty() {
            raw_lines.push(String::new());
            consumed_lines += 1;
            continue;
        }

        break;
    }

    let start = raw_lines
        .iter()
        .position(|line| !line.is_empty())
        .unwrap_or(0);
    let end = raw_lines
        .iter()
        .rposition(|line| !line.is_empty())
        .map(|index| index + 1)
        .unwrap_or(0);
    if start >= end {
        return None;
    }

    let value = format!("{}\n", raw_lines[start..end].join("\n"));

    Some(IndentedCodeToken {
        consumed_lines,
        value,
    })
}

fn is_indented_line(line: &str) -> bool {
    leading_indent_columns(line) >= 4
}

fn strip_indented_prefix(line: &str) -> Option<String> {
    strip_indent_columns(line, 4)
}
