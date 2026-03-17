use yozora_ast::{Footnote, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const FOOTNOTE_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote";

#[derive(Debug, Clone)]
pub struct FootnoteTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FOOTNOTE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                // containing-inline 语义需要在 emphasis / inline-code 之前处理。
                priority: 13,
            },
        }
    }
}

impl Tokenizer for FootnoteTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for FootnoteTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains("^[") {
            return None;
        }

        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut matched = false;

        while let Some(offset) = input[cursor..].find("^[") {
            let start = cursor + offset;
            if is_escaped(input, start) {
                let escape_start = start.saturating_sub(1);
                if escape_start > cursor {
                    nodes.push(Node::Text(Text {
                        position: None,
                        value: input[cursor..escape_start].to_string(),
                    }));
                }

                nodes.push(Node::Text(Text {
                    position: None,
                    value: "^[".to_string(),
                }));

                matched = true;
                cursor = start + 2;
                continue;
            }

            let content_start = start + 2;
            let Some(end) = find_matching_bracket(input, content_start) else {
                break;
            };

            if start > cursor {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[cursor..start].to_string(),
                }));
            }

            nodes.push(Node::Footnote(Footnote {
                position: None,
                children: vec![Node::Text(Text {
                    position: None,
                    value: input[content_start..end].to_string(),
                })],
            }));

            matched = true;
            cursor = end + 1;
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

fn find_matching_bracket(input: &str, content_start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut depth = 1usize;
    let mut index = content_start;
    let mut code_ticks: usize = 0;

    while index < bytes.len() {
        if code_ticks > 0 {
            if bytes[index] == b'`' {
                let run = count_repeat(bytes, index, b'`');
                if run >= code_ticks {
                    code_ticks = 0;
                }
                index += run;
                continue;
            }

            index += 1;
            continue;
        }

        match bytes[index] {
            b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            b'`' => {
                code_ticks = count_repeat(bytes, index, b'`');
                index += code_ticks;
            }
            b'[' => {
                depth += 1;
                index += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
                index += 1;
            }
            _ => index += 1,
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
