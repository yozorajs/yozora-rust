use yozora_character::{NodePoint, VirtualCodePoint};

pub(crate) fn match_soft_break_text(input: &str) -> Option<String> {
    if !input.contains('\n') {
        return None;
    }

    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        if bytes[i] != b'\n' {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }

        while out.ends_with(' ') || out.ends_with('\t') {
            out.pop();
            changed = true;
        }

        out.push('\n');
        i += 1;

        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
            changed = true;
        }
    }

    if !changed {
        return None;
    }

    Some(out)
}

pub(crate) fn match_soft_break_value(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<String> {
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

    match_soft_break_text(&source)
}
