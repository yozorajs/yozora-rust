use yozora_ast::{BreakNode, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-break";

#[derive(Debug, Clone)]
pub struct BreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for BreakTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: BREAK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 8,
            },
        }
    }
}

impl Tokenizer for BreakTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for BreakTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('\n') {
            return None;
        }

        let bytes = input.as_bytes();
        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut index = 0usize;
        let mut matched = false;

        while index < bytes.len() {
            if bytes[index] != b'\n' {
                index += 1;
                continue;
            }

            let marker_start = if index > 0 && bytes[index - 1] == b'\\' {
                Some(index - 1)
            } else {
                let mut spaces_start = index;
                while spaces_start > 0 && bytes[spaces_start - 1] == b' ' {
                    spaces_start -= 1;
                }
                if index - spaces_start >= 2 {
                    Some(spaces_start)
                } else {
                    None
                }
            };

            let Some(marker_start) = marker_start else {
                index += 1;
                continue;
            };

            if marker_start > cursor {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[cursor..marker_start].to_string(),
                }));
            }
            nodes.push(Node::Break(BreakNode { position: None }));

            matched = true;
            cursor = index;
            index += 1;
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
