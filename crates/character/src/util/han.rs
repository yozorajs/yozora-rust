fn is_han_or_common_punct(ch: char) -> bool {
    matches!(
        ch,
        '\u{3002}'
            | '\u{ff1f}'
            | '\u{ff01}'
            | '\u{ff0c}'
            | '\u{3001}'
            | '\u{ff1b}'
            | '\u{ff1a}'
            | '\u{201c}'
            | '\u{201d}'
            | '\u{2018}'
            | '\u{2019}'
            | '\u{ff08}'
            | '\u{ff09}'
            | '\u{300a}'
            | '\u{300b}'
            | '\u{3008}'
            | '\u{3009}'
            | '\u{3010}'
            | '\u{3011}'
            | '\u{300e}'
            | '\u{300f}'
            | '\u{300c}'
            | '\u{300d}'
            | '\u{fe43}'
            | '\u{fe44}'
            | '\u{3014}'
            | '\u{3015}'
            | '\u{2026}'
            | '\u{2014}'
            | '\u{ff5e}'
            | '\u{fe4f}'
            | '\u{ffe5}'
    ) || ('\u{4E00}'..='\u{9FFF}').contains(&ch)
}

pub fn strip_chinese_characters(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let chars: Vec<char> = value.chars().collect();

    for i in 0..chars.len() {
        let ch = chars[i];
        if ch == '\n' {
            let prev = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
            let next = chars.get(i + 1).copied();
            if let (Some(a), Some(b)) = (prev, next) {
                if is_han_or_common_punct(a) && is_han_or_common_punct(b) {
                    continue;
                }
            }
        }
        out.push(ch);
    }

    out
}

pub fn tighten_chinese_characters(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let chars: Vec<char> = value.chars().collect();

    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            let prev = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let next = chars.get(j).copied();
            if let (Some(a), Some(b)) = (prev, next) {
                if is_han_or_common_punct(a) && is_han_or_common_punct(b) {
                    i = j;
                    continue;
                }
            }
        }

        out.push(ch);
        i += 1;
    }

    out
}
