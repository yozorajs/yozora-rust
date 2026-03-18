#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetextHeadingToken {
    pub consumed_lines: usize,
    pub depth: u8,
    pub content: String,
}

pub(crate) fn match_setext_heading_token(lines: &[&str]) -> Option<SetextHeadingToken> {
    if lines.len() < 2 {
        return None;
    }

    if leading_space_count(lines[0]) >= 4 {
        return None;
    }

    let mut content_lines = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            if line.trim().is_empty() {
                return None;
            }
            content_lines.push(trim_line_end(line));
            continue;
        }

        if let Some(depth) = parse_underline_depth(line) {
            let content = content_lines.join("\n");
            let content = content.trim().to_string();
            if content.is_empty() {
                return None;
            }

            return Some(SetextHeadingToken {
                consumed_lines: index + 1,
                depth,
                content,
            });
        }

        if line.trim().is_empty() || leading_space_count(line) >= 4 {
            break;
        }

        content_lines.push(trim_line_end(line));
    }

    None
}

fn parse_underline_depth(line: &str) -> Option<u8> {
    if leading_space_count(line) >= 4 {
        return None;
    }

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let marker = trimmed.chars().next()?;
    if marker != '=' && marker != '-' {
        return None;
    }

    if !trimmed.chars().all(|ch| ch == marker) {
        return None;
    }

    Some(if marker == '=' { 1 } else { 2 })
}

fn trim_line_end(line: &str) -> String {
    line.trim_end().to_string()
}

fn leading_space_count(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}
