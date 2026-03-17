use yozora_ast::{InlineMath, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const INLINE_MATH_TOKENIZER_NAME: &str = "@yozora/tokenizer-inline-math";

#[derive(Debug, Clone)]
pub struct InlineMathTokenizer {
    meta: TokenizerMeta,
}

impl Default for InlineMathTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: INLINE_MATH_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 11,
            },
        }
    }
}

impl Tokenizer for InlineMathTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for InlineMathTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('$') {
            return None;
        }

        let bytes = input.as_bytes();
        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut matched = false;

        while cursor < bytes.len() {
            let Some((start, end, value)) = find_next_inline_math(input, cursor) else {
                if cursor < input.len() {
                    nodes.push(Node::Text(Text {
                        position: None,
                        value: input[cursor..].to_string(),
                    }));
                }
                break;
            };

            if start > cursor {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[cursor..start].to_string(),
                }));
            }

            nodes.push(Node::InlineMath(InlineMath {
                position: None,
                value,
            }));
            matched = true;
            cursor = end;
        }

        if !matched {
            return None;
        }

        Some(merge_adjacent_text(nodes))
    }
}

fn find_next_inline_math(input: &str, from: usize) -> Option<(usize, usize, String)> {
    let bytes = input.as_bytes();
    let mut index = from;

    while index < bytes.len() {
        match bytes[index] {
            b'`' => {
                if let Some(result) = parse_backtick_wrapped_math(input, index) {
                    return Some(result);
                }
            }
            b'$' => {
                if let Some(result) = parse_plain_math(input, index) {
                    return Some(result);
                }
            }
            _ => {}
        }
        index += 1;
    }

    None
}

fn parse_backtick_wrapped_math(input: &str, start: usize) -> Option<(usize, usize, String)> {
    if is_escaped(input, start) {
        return None;
    }

    if start > 0 && input.as_bytes()[start - 1] == b'`' {
        return None;
    }

    let bytes = input.as_bytes();
    let tick_len = count_run(bytes, start, b'`');
    let dollar_start = start + tick_len;
    let dollar_len = count_run(bytes, dollar_start, b'$');
    if dollar_len == 0 {
        return None;
    }

    let content_start = dollar_start + dollar_len;
    let mut index = content_start;

    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }

        if bytes[index] == b'$' {
            let run = count_run(bytes, index, b'$');
            if run == dollar_len {
                let tick_start = index + dollar_len;
                if count_run(bytes, tick_start, b'`') == tick_len {
                    if index <= content_start {
                        return None;
                    }

                    let content = &input[content_start..index];
                    return Some((
                        start,
                        tick_start + tick_len,
                        normalize_inline_math_content(content),
                    ));
                }
            }
            index += run.max(1);
            continue;
        }

        index += 1;
    }

    None
}

fn parse_plain_math(input: &str, start: usize) -> Option<(usize, usize, String)> {
    if is_escaped(input, start) {
        return None;
    }

    let bytes = input.as_bytes();
    let dollar_len = count_run(bytes, start, b'$');
    if dollar_len == 0 {
        return None;
    }

    if start > 0 && bytes[start - 1] == b'`' {
        return None;
    }

    let content_start = start + dollar_len;
    if content_start >= bytes.len() {
        return None;
    }

    if bytes[content_start].is_ascii_digit() {
        return None;
    }

    let mut index = content_start;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }

        if bytes[index] == b'$' {
            let run = count_run(bytes, index, b'$');
            if run == dollar_len {
                if index <= content_start {
                    return None;
                }

                let end = index + dollar_len;
                if end < bytes.len() && bytes[end] == b'`' {
                    return None;
                }

                let content = &input[content_start..index];
                if content.contains('`') {
                    return None;
                }

                return Some((start, end, normalize_inline_math_content(content)));
            }
            index += run.max(1);
            continue;
        }

        index += 1;
    }

    None
}

fn normalize_inline_math_content(content: &str) -> String {
    if !content.contains('\n') && !content.contains('\r') {
        return content.to_string();
    }

    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn count_run(bytes: &[u8], mut index: usize, target: u8) -> usize {
    let mut len = 0usize;
    while index < bytes.len() && bytes[index] == target {
        index += 1;
        len += 1;
    }
    len
}

fn is_escaped(input: &str, byte_index: usize) -> bool {
    if byte_index == 0 {
        return false;
    }

    let bytes = input.as_bytes();
    let mut index = byte_index;
    let mut slash_count = 0usize;
    while index > 0 {
        index -= 1;
        if bytes[index] == b'\\' {
            slash_count += 1;
        } else {
            break;
        }
    }

    slash_count % 2 == 1
}

fn merge_adjacent_text(nodes: Vec<Node>) -> Vec<Node> {
    let mut merged = Vec::new();
    for node in nodes {
        match node {
            Node::Text(text) => {
                if let Some(Node::Text(last)) = merged.last_mut() {
                    last.value.push_str(&text.value);
                } else {
                    merged.push(Node::Text(text));
                }
            }
            other => merged.push(other),
        }
    }
    merged
}
