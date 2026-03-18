#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThematicBreakToken;

pub(crate) fn match_thematic_break_token(input: &str) -> Option<ThematicBreakToken> {
    if input.contains('\n') {
        return None;
    }

    let leading_spaces = input.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return None;
    }

    let line = input.trim();
    if line.is_empty() {
        return None;
    }

    let mut marker: Option<char> = None;
    let mut count = 0usize;
    for ch in line.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }

        if ch != '-' && ch != '*' && ch != '_' {
            return None;
        }

        if let Some(m) = marker {
            if m != ch {
                return None;
            }
        } else {
            marker = Some(ch);
        }
        count += 1;
    }

    if count < 3 {
        return None;
    }

    Some(ThematicBreakToken)
}
