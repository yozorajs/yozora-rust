pub fn leading_indent_columns(line: &str) -> usize {
    split_indent_prefix(line, 0).0
}

pub fn split_indent_prefix(line: &str, base_column: usize) -> (usize, usize) {
    let mut column = base_column;
    let mut consumed_bytes = 0usize;

    for (idx, ch) in line.char_indices() {
        let width = match ch {
            ' ' => 1,
            '\t' => 4 - (column % 4),
            _ => break,
        };

        column += width;
        consumed_bytes = idx + ch.len_utf8();
    }

    (column.saturating_sub(base_column), consumed_bytes)
}

pub fn strip_indent_columns(line: &str, columns: usize) -> Option<String> {
    strip_indent_columns_with_base(line, columns, 0)
}

pub fn strip_indent_columns_with_base(
    line: &str,
    columns: usize,
    base_column: usize,
) -> Option<String> {
    if columns == 0 {
        return Some(line.to_string());
    }

    let mut column = base_column;
    let mut content_start = 0usize;

    for (idx, ch) in line.char_indices() {
        let width = match ch {
            ' ' => 1,
            '\t' => 4 - (column % 4),
            _ => {
                content_start = idx;
                break;
            }
        };

        column += width;
        content_start = idx + ch.len_utf8();
    }

    let total_indent = column.saturating_sub(base_column);
    if total_indent < columns {
        return None;
    }

    let remaining_indent = total_indent - columns;
    Some(format!(
        "{}{}",
        " ".repeat(remaining_indent),
        &line[content_start..]
    ))
}
