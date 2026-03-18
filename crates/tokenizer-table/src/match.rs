use yozora_ast::AlignType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableToken {
    pub consumed_lines: usize,
    pub alignments: Vec<Option<AlignType>>,
    pub rows: Vec<Vec<String>>,
}

pub(crate) fn match_table_token(lines: &[&str]) -> Option<TableToken> {
    if lines.len() < 2 {
        return None;
    }

    if !lines[0].contains('|') && !lines[1].contains('|') {
        return None;
    }

    let header_cells = split_table_cells(lines[0]);
    if header_cells.is_empty() {
        return None;
    }

    let alignments = parse_delimiter_row(lines[1])?;
    if alignments.is_empty() {
        return None;
    }

    if header_cells.len() != alignments.len() {
        return None;
    }

    let column_count = alignments.len();
    let mut rows = vec![normalize_row_cells(&header_cells, column_count)];
    let mut consumed_lines = 2usize;

    for line in lines.iter().skip(2) {
        if line.trim().is_empty() {
            break;
        }
        if starts_blockquote(line) {
            break;
        }
        let cells = split_table_cells(line);
        rows.push(normalize_row_cells(&cells, column_count));
        consumed_lines += 1;
    }

    Some(TableToken {
        consumed_lines,
        alignments,
        rows,
    })
}

fn parse_delimiter_row(line: &str) -> Option<Vec<Option<AlignType>>> {
    let cells = split_table_cells(line);
    if cells.is_empty() {
        return None;
    }

    let mut alignments = Vec::new();
    for cell in cells {
        let trimmed = cell.trim();
        if trimmed.is_empty() {
            return None;
        }

        let align = if trimmed.starts_with(':') && trimmed.ends_with(':') && trimmed.len() >= 3 {
            Some(AlignType::Center)
        } else if trimmed.starts_with(':') && trimmed.len() >= 2 {
            Some(AlignType::Left)
        } else if trimmed.ends_with(':') && trimmed.len() >= 2 {
            Some(AlignType::Right)
        } else {
            None
        };

        let mut bar = trimmed;
        if let Some(rest) = bar.strip_prefix(':') {
            bar = rest;
        }
        if let Some(rest) = bar.strip_suffix(':') {
            bar = rest;
        }
        if bar.is_empty() || !bar.chars().all(|ch| ch == '-') {
            return None;
        }

        alignments.push(align);
    }

    Some(alignments)
}

fn split_table_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escape = false;

    for ch in line.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        if ch == '\\' {
            escape = true;
            continue;
        }

        if ch == '|' {
            cells.push(current.trim().to_string());
            current.clear();
            continue;
        }

        current.push(ch);
    }
    cells.push(current.trim().to_string());

    if line.trim_start().starts_with('|') && !cells.is_empty() {
        cells.remove(0);
    }
    if line.trim_end().ends_with('|') && !cells.is_empty() {
        cells.pop();
    }

    cells
}

fn normalize_row_cells(cells: &[String], column_count: usize) -> Vec<String> {
    (0..column_count)
        .map(|index| cells.get(index).cloned().unwrap_or_default())
        .collect()
}

fn starts_blockquote(line: &str) -> bool {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return false;
    }

    line[leading_spaces..].starts_with('>')
}
