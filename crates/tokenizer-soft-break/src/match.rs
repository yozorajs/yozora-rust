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
