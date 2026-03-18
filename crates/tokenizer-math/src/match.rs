#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MathToken {
    pub consumed_lines: usize,
    pub value: String,
}

pub(crate) fn match_math_token(lines: &[&str]) -> Option<MathToken> {
    let first = *lines.first()?;
    let trimmed = first.trim();
    let opening_len = trimmed.chars().take_while(|ch| *ch == '$').count();
    if opening_len < 2 {
        return None;
    }

    // Single-line math fence: opening and closing fence lengths must match.
    if !trimmed.chars().all(|ch| ch == '$') {
        let closing_len = trimmed.chars().rev().take_while(|ch| *ch == '$').count();
        if closing_len != opening_len || trimmed.len() <= opening_len + closing_len {
            return None;
        }

        let content = trimmed[opening_len..trimmed.len() - closing_len].trim();
        if content.is_empty() {
            return None;
        }

        return Some(MathToken {
            consumed_lines: 1,
            value: format!("{}\n", content),
        });
    }

    let fence = "$".repeat(opening_len);
    let mut content = Vec::new();
    let mut consumed_lines = lines.len();

    for (idx, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == fence {
            consumed_lines = idx + 1;
            break;
        }
        content.push((*line).to_string());
    }

    let value = if content.is_empty() {
        String::new()
    } else {
        format!("{}\n", content.join("\n"))
    };

    Some(MathToken {
        consumed_lines,
        value,
    })
}
