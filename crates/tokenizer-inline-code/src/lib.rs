use yozora_ast::{InlineCode, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const INLINE_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-inline-code";

#[derive(Debug, Clone)]
pub struct InlineCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for InlineCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: INLINE_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 10,
            },
        }
    }
}

impl Tokenizer for InlineCodeTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for InlineCodeTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('`') {
            return None;
        }

        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut matched = false;
        let bytes = input.as_bytes();
        let mut scan = 0usize;

        while scan < bytes.len() {
            if bytes[scan] != b'`' {
                scan += 1;
                continue;
            }

            let start = scan;
            let opener_len = count_backtick_run(bytes, start);
            if is_escaped(input, start) {
                scan += opener_len;
                continue;
            }

            if is_inside_angle_segment(input, start) {
                scan += opener_len;
                continue;
            }

            let Some((end_start, end)) =
                find_matching_backtick_run(bytes, start + opener_len, opener_len)
            else {
                scan += opener_len;
                continue;
            };

            if has_unmatched_open_bracket_before(input, start)
                && has_link_closer_after(input, end)
            {
                scan = start + 1;
                continue;
            }

            if has_unmatched_open_bracket_before(input, start)
                && is_inside_unmatched_emphasis(input, start)
            {
                scan = start + 1;
                continue;
            }

            if start > cursor {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[cursor..start].to_string(),
                }));
            }

            nodes.push(Node::InlineCode(InlineCode {
                position: None,
                value: normalize_code_span(&input[start + opener_len..end_start]),
            }));
            matched = true;
            cursor = end;
            scan = end;
        }

        if !matched {
            return None;
        }

        if cursor < input.len() {
            nodes.push(Node::Text(Text {
                position: None,
                value: input[cursor..].to_string(),
            }));
        }

        Some(nodes)
    }
}

fn count_backtick_run(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    while index < bytes.len() && bytes[index] == b'`' {
        index += 1;
    }
    index - start
}

fn find_matching_backtick_run(
    bytes: &[u8],
    mut search_from: usize,
    opener_len: usize,
) -> Option<(usize, usize)> {
    while search_from < bytes.len() {
        if bytes[search_from] != b'`' {
            search_from += 1;
            continue;
        }

        let len = count_backtick_run(bytes, search_from);
        if len == opener_len {
            return Some((search_from, search_from + len));
        }

        search_from += len;
    }

    None
}

fn normalize_code_span(raw: &str) -> String {
    let collapsed = raw.replace("\r\n", "\n").replace('\r', "\n").replace('\n', " ");

    let is_all_space = collapsed.chars().all(|ch| ch == ' ');
    if !is_all_space && collapsed.len() >= 2 && collapsed.starts_with(' ') && collapsed.ends_with(' ') {
        return collapsed[1..collapsed.len() - 1].to_string();
    }

    collapsed
}

fn is_inside_angle_segment(input: &str, byte_index: usize) -> bool {
    let bytes = input.as_bytes();
    if byte_index > bytes.len() {
        return false;
    }

    let mut has_open_angle = false;
    let mut index = byte_index;
    while index > 0 {
        index -= 1;
        match bytes[index] {
            b'>' => return false,
            b'<' => {
                has_open_angle = true;
                break;
            }
            _ => {}
        }
    }

    if !has_open_angle {
        return false;
    }

    for &ch in &bytes[byte_index..] {
        match ch {
            b'>' => return true,
            b'<' | b'\n' => return false,
            _ => {}
        }
    }

    false
}

fn is_escaped(input: &str, byte_index: usize) -> bool {
    if byte_index == 0 {
        return false;
    }

    let bytes = input.as_bytes();
    let mut backslash_count = 0usize;
    let mut index = byte_index;
    while index > 0 {
        index -= 1;
        if bytes[index] != b'\\' {
            break;
        }
        backslash_count += 1;
    }

    backslash_count % 2 == 1
}

fn has_unmatched_open_bracket_before(input: &str, end: usize) -> bool {
    let bytes = input.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;

    while i < end && i < bytes.len() {
        match bytes[i] {
            b'\\' => i = (i + 2).min(end),
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                if depth > 0 {
                    depth -= 1;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }

    depth > 0
}

fn has_link_closer_after(input: &str, start: usize) -> bool {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return false;
    }

    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == b']' && (bytes[i + 1] == b'(' || bytes[i + 1] == b'[') {
            return true;
        }
        i += 1;
    }

    false
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
