use yozora_ast::{LinkReference, Node, ReferenceType, Text};
use yozora_character::fold_case;
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const LINK_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-link-reference";

#[derive(Debug, Clone)]
pub struct LinkReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for LinkReferenceTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: LINK_REFERENCE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 6,
            },
        }
    }
}

impl Tokenizer for LinkReferenceTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for LinkReferenceTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('[') {
            return None;
        }

        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut last_emit = 0usize;
        let mut matched = false;

        while let Some(offset) = input[cursor..].find('[') {
            let start = cursor + offset;

            if is_escaped(input, start)
                || (start > 0
                    && input.as_bytes()[start - 1] == b'!'
                    && !is_escaped(input, start - 1))
            {
                cursor = start + 1;
                continue;
            }
            let preceded_by_emphasis_marker =
                start > 0 && matches!(input.as_bytes()[start - 1], b'*' | b'_');
            if is_inside_unmatched_emphasis(input, start) && !preceded_by_emphasis_marker {
                cursor = start + 1;
                continue;
            }

            let Some((first_close, first_raw)) = parse_link_text(input, start) else {
                cursor = start + 1;
                continue;
            };

            let first_label_raw = first_raw.to_string();
            if first_label_raw.trim().is_empty() {
                cursor = start + 1;
                continue;
            }

            if contains_valid_inline_link(first_raw) || contains_valid_link_reference(first_raw) {
                // Links may not contain links. Prefer parsing inner link-like structures.
                cursor = start + 1;
                continue;
            }

            let first_text = decode_escapes(first_raw);

            let after_first = first_close + 1;
            if after_first < input.len()
                && input.as_bytes()[after_first] == b'('
                && has_valid_inline_link_after(input, after_first)
            {
                // Valid inline link has higher precedence.
                cursor = start + 1;
                continue;
            }

            let (identifier, label, reference_type, consumed_len) =
                if after_first < input.len() && input.as_bytes()[after_first] == b'[' {
                    let Some((second_close, second_raw, second_has_non_ws)) =
                        parse_link_label(input, after_first)
                    else {
                        cursor = start + 1;
                        continue;
                    };

                    let consumed = second_close + 1 - start;
                    if second_has_non_ws {
                        let second_label = second_raw.to_string();
                        (
                            normalize_identifier(&second_label),
                            second_label,
                            ReferenceType::Full,
                            consumed,
                        )
                    } else {
                        // Yozora custom supplementary accepts `[foo][  ]` as collapsed.
                        (
                            normalize_identifier(&first_label_raw),
                            first_label_raw.clone(),
                            ReferenceType::Collapsed,
                            consumed,
                        )
                    }
                } else {
                    (
                        normalize_identifier(&first_label_raw),
                        first_label_raw.clone(),
                        ReferenceType::Shortcut,
                        first_close + 1 - start,
                    )
                };

            if reference_type == ReferenceType::Shortcut
                && first_label_raw.starts_with('[')
                && first_label_raw.ends_with(']')
            {
                // For nested brackets like `[[*foo* bar]]`, defer to inner shortcut label.
                cursor = start + 1;
                continue;
            }

            if reference_type == ReferenceType::Shortcut
                && followed_by_outer_inline_link_closer(input, after_first)
            {
                // Prefer the outer inline-link pair over an inner shortcut reference.
                cursor = start + 1;
                continue;
            }

            if start > last_emit {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[last_emit..start].to_string(),
                }));
            }

            nodes.push(Node::LinkReference(LinkReference {
                position: None,
                identifier,
                label,
                reference_type,
                children: vec![Node::Text(Text {
                    position: None,
                    value: first_text,
                })],
            }));

            matched = true;
            cursor = start + consumed_len;
            last_emit = cursor;
        }

        if !matched {
            return None;
        }

        if last_emit < input.len() {
            nodes.push(Node::Text(Text {
                position: None,
                value: input[last_emit..].to_string(),
            }));
        }

        Some(nodes)
    }
}

fn parse_link_text(input: &str, start: usize) -> Option<(usize, &str)> {
    if input.as_bytes().get(start).copied()? != b'[' {
        return None;
    }

    let bytes = input.as_bytes();
    let mut i = start + 1;
    let mut depth = 1usize;
    let mut code_ticks = 0usize;

    while i < bytes.len() {
        if code_ticks > 0 {
            if bytes[i] == b'`' {
                let run = count_repeat(bytes, i, b'`');
                if run >= code_ticks {
                    code_ticks = 0;
                }
                i += run;
                continue;
            }
            i += 1;
            continue;
        }

        match bytes[i] {
            b'\\' => i = (i + 2).min(bytes.len()),
            b'`' => {
                code_ticks = count_repeat(bytes, i, b'`');
                i += code_ticks;
            }
            b'<' => {
                if let Some(next) = skip_angle_segment(bytes, i) {
                    i = next;
                } else {
                    i += 1;
                }
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((i, &input[start + 1..i]));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }

    None
}

fn parse_link_label(input: &str, start: usize) -> Option<(usize, &str, bool)> {
    if input.as_bytes().get(start).copied()? != b'[' {
        return None;
    }

    let bytes = input.as_bytes();
    let mut i = start + 1;
    let mut char_count = 0usize;
    let mut has_non_whitespace = false;

    while i < bytes.len() {
        char_count += 1;
        if char_count > 1000 {
            return None;
        }

        match bytes[i] {
            b'\\' => {
                if i + 1 >= bytes.len() {
                    return None;
                }
                has_non_whitespace = true;
                i += 2;
            }
            b'[' => return None,
            b']' => return Some((i, &input[start + 1..i], has_non_whitespace)),
            ch => {
                if ch != 0x1F && !(ch as char).is_whitespace() {
                    has_non_whitespace = true;
                }
                i += 1;
            }
        }
    }

    None
}

fn count_repeat(bytes: &[u8], mut index: usize, target: u8) -> usize {
    let mut count = 0usize;
    while index < bytes.len() && bytes[index] == target {
        count += 1;
        index += 1;
    }
    count
}

fn skip_angle_segment(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start).copied()? != b'<' {
        return None;
    }

    let mut i = start + 1;
    let mut quote: Option<u8> = None;

    while i < bytes.len() {
        if let Some(q) = quote {
            match bytes[i] {
                b'\\' => i = (i + 2).min(bytes.len()),
                b if b == q => {
                    quote = None;
                    i += 1;
                }
                _ => i += 1,
            }
            continue;
        }

        match bytes[i] {
            b'"' | b'\'' => {
                quote = Some(bytes[i]);
                i += 1;
            }
            b'>' => return Some(i + 1),
            b'\n' | b'\r' => return None,
            _ => i += 1,
        }
    }

    None
}

fn is_escaped(input: &str, byte_index: usize) -> bool {
    if byte_index == 0 {
        return false;
    }

    let bytes = input.as_bytes();
    let mut idx = byte_index;
    let mut slash_count = 0usize;
    while idx > 0 {
        idx -= 1;
        if bytes[idx] == b'\\' {
            slash_count += 1;
        } else {
            break;
        }
    }

    slash_count % 2 == 1
}

fn is_inside_unmatched_emphasis(input: &str, byte_index: usize) -> bool {
    let bytes = input.as_bytes();
    let mut star_count = 0usize;
    let mut underscore_count = 0usize;
    let mut i = 0usize;

    while i < byte_index && i < bytes.len() {
        if bytes[i] == b'\\' {
            i = (i + 2).min(byte_index);
            continue;
        }

        if bytes[i] == b'*' {
            star_count += 1;
        } else if bytes[i] == b'_' {
            underscore_count += 1;
        }

        i += 1;
    }

    star_count % 2 == 1 || underscore_count % 2 == 1
}

fn decode_escapes(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                if is_escapable_char(next) {
                    out.push(next);
                } else {
                    out.push('\\');
                    out.push(next);
                }
            } else {
                out.push('\\');
            }
            continue;
        }
        out.push(ch);
    }

    out
}

fn normalize_identifier(label: &str) -> String {
    let mut collapsed = String::new();
    for (idx, part) in label.split_whitespace().enumerate() {
        if idx > 0 {
            collapsed.push(' ');
        }
        collapsed.push_str(part);
    }
    fold_case(&collapsed)
}

fn is_escapable_char(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

fn has_valid_inline_link_after(input: &str, open_paren: usize) -> bool {
    parse_inline_link_tail(input, open_paren).is_some()
}

fn contains_valid_inline_link(input: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(offset) = input[cursor..].find('[') {
        let start = cursor + offset;
        if is_escaped(input, start) || (start > 0 && input.as_bytes()[start - 1] == b'!') {
            cursor = start + 1;
            continue;
        }

        let Some((label_end, _)) = parse_link_text(input, start) else {
            cursor = start + 1;
            continue;
        };

        let open_paren = label_end + 1;
        if input.as_bytes().get(open_paren).copied() != Some(b'(') {
            cursor = start + 1;
            continue;
        }

        if parse_inline_link_tail(input, open_paren).is_some() {
            return true;
        }

        cursor = start + 1;
    }

    false
}

fn contains_valid_link_reference(input: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(offset) = input[cursor..].find('[') {
        let start = cursor + offset;
        if is_escaped(input, start) || (start > 0 && input.as_bytes()[start - 1] == b'!') {
            cursor = start + 1;
            continue;
        }

        let Some((first_close, first_raw)) = parse_link_text(input, start) else {
            cursor = start + 1;
            continue;
        };
        if first_raw.trim().is_empty() {
            cursor = start + 1;
            continue;
        }

        let after_first = first_close + 1;
        if input.as_bytes().get(after_first).copied() != Some(b'[') {
            cursor = start + 1;
            continue;
        }

        if parse_link_label(input, after_first).is_some() {
            return true;
        }

        cursor = start + 1;
    }

    false
}

fn parse_inline_link_tail(input: &str, open_paren: usize) -> Option<usize> {
    if input.as_bytes().get(open_paren).copied()? != b'(' {
        return None;
    }

    let bytes = input.as_bytes();
    let len = bytes.len();

    let i = eat_optional_ascii_whitespace(bytes, open_paren + 1);
    if i >= len {
        return None;
    }

    if bytes[i] == b')' {
        return Some(i + 1);
    }

    let destination_end = parse_link_destination(input, i)?;
    let title_start = eat_optional_ascii_whitespace(bytes, destination_end);
    let has_separating_whitespace = title_start > destination_end;

    let title_end = if bytes.get(title_start).copied() == Some(b')') {
        title_start
    } else {
        if !has_separating_whitespace {
            return None;
        }
        parse_link_title(input, title_start)?
    };

    let close_index = eat_optional_ascii_whitespace(bytes, title_end);
    if bytes.get(close_index).copied() != Some(b')') {
        return None;
    }

    Some(close_index + 1)
}

fn parse_link_destination(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return None;
    }

    if bytes[start] == b'<' {
        let mut i = start + 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i = (i + 2).min(bytes.len()),
                b'<' | b'\n' | b'\r' => return None,
                b'>' => return Some(i + 1),
                _ => i += 1,
            }
        }
        return None;
    }

    let mut i = start;
    let mut open_parens = 0i32;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b.is_ascii_control() {
            break;
        }

        match b {
            b'\\' => i = (i + 2).min(bytes.len()),
            b'(' => {
                open_parens += 1;
                i += 1;
            }
            b')' => {
                if open_parens == 0 {
                    break;
                }
                open_parens -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }

    if i == start || open_parens != 0 {
        return None;
    }

    Some(i)
}

fn parse_link_title(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let open = *bytes.get(start)?;

    if open == b')' {
        return Some(start);
    }

    match open {
        b'"' | b'\'' => {
            let mut i = start + 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i = (i + 2).min(bytes.len()),
                    b if b == open => {
                        if contains_blank_line(&input[start + 1..i]) {
                            return None;
                        }
                        return Some(i + 1);
                    }
                    _ => i += 1,
                }
            }
            None
        }
        b'(' => {
            let mut i = start + 1;
            let mut open_parens = 1i32;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i = (i + 2).min(bytes.len()),
                    b'(' => {
                        open_parens += 1;
                        i += 1;
                    }
                    b')' => {
                        open_parens -= 1;
                        if open_parens == 0 {
                            if contains_blank_line(&input[start + 1..i]) {
                                return None;
                            }
                            return Some(i + 1);
                        }
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
            None
        }
        _ => None,
    }
}

fn eat_optional_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn contains_blank_line(raw: &str) -> bool {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let bytes = normalized.as_bytes();

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'\n' {
            i += 1;
            continue;
        }

        let mut j = i + 1;
        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
            j += 1;
        }

        if j < bytes.len() && bytes[j] == b'\n' {
            return true;
        }

        i = j;
    }

    false
}

fn followed_by_outer_inline_link_closer(input: &str, mut index: usize) -> bool {
    let bytes = input.as_bytes();
    if index >= bytes.len() || bytes[index] != b']' {
        return false;
    }

    while index < bytes.len() && bytes[index] == b']' {
        index += 1;
    }

    index < bytes.len() && bytes[index] == b'(' && has_valid_inline_link_after(input, index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_not_capture_inline_link_with_nested_brackets() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer.tokenize_inline("[link [foo [bar]]](/uri)", None);
        assert!(nodes.is_none(), "got: {:?}", nodes);
    }

    #[test]
    fn should_not_capture_inline_link_with_image_label() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer.tokenize_inline("[![moon](moon.jpg)](/uri)", None);
        assert!(nodes.is_none());
    }

    #[test]
    fn should_capture_full_reference_with_inline_content() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer.tokenize_inline("[link *foo **bar** `#`*][ref]", None);
        assert!(nodes.is_some(), "nodes should be parsed");

        let nodes = nodes.unwrap();
        assert_eq!(nodes.len(), 1, "got: {nodes:?}");
        let Node::LinkReference(link) = &nodes[0] else {
            panic!("expected linkReference, got: {nodes:?}");
        };
        assert_eq!(link.reference_type, ReferenceType::Full);
        assert_eq!(link.label, "ref");
    }

    #[test]
    fn should_prefer_middle_full_reference_in_three_labels() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[foo][bar][baz]", None)
            .expect("nodes");

        let Node::LinkReference(link) = &nodes[0] else {
            panic!("got: {nodes:?}");
        };
        assert_eq!(link.reference_type, ReferenceType::Full);
        assert_eq!(link.label, "bar");
    }

    #[test]
    fn should_parse_inner_shortcut_for_double_wrapped_label() {
        let tokenizer = LinkReferenceTokenizer::default();
        let nodes = tokenizer
            .tokenize_inline("[[*foo* bar]]", None)
            .expect("nodes");

        assert!(matches!(&nodes[0], Node::Text(Text { value, .. }) if value == "["));
        assert!(matches!(&nodes[1], Node::LinkReference(_)), "got: {nodes:?}");
        assert!(matches!(&nodes[2], Node::Text(Text { value, .. }) if value == "]"));
    }
}
