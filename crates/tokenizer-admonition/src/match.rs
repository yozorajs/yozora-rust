#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdmonitionBlockToken {
    pub keyword: String,
    pub title: String,
    pub body_lines: Vec<String>,
    pub consumed_lines: usize,
}

pub(crate) fn match_admonition(lines: &[&str]) -> Option<AdmonitionBlockToken> {
    let first = *lines.first()?;
    let (keyword, title) = parse_opening_admonition(first)?;

    let mut consumed_lines = 1usize;
    let mut body_lines = Vec::new();
    let mut has_closing = false;

    for line in lines.iter().skip(1) {
        if line.trim() == ":::" {
            consumed_lines += 1;
            has_closing = true;
            break;
        }

        body_lines.push((*line).to_string());
        consumed_lines += 1;
    }

    if !has_closing {
        return None;
    }

    Some(AdmonitionBlockToken {
        keyword,
        title,
        body_lines,
        consumed_lines,
    })
}

fn parse_opening_admonition(line: &str) -> Option<(String, String)> {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return None;
    }

    let trimmed = line[leading_spaces..].trim_end();
    if !trimmed.starts_with(":::") {
        return None;
    }

    let rest = trimmed[3..].trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let keyword = parts.next()?.trim();
    if keyword.is_empty() {
        return None;
    }

    let title = parts.next().map(str::trim).unwrap_or_default().to_string();
    Some((keyword.to_string(), title))
}
