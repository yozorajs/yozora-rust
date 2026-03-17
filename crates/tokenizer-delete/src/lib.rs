use yozora_ast::{DeleteNode, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const DELETE_TOKENIZER_NAME: &str = "@yozora/tokenizer-delete";

#[derive(Debug, Clone)]
pub struct DeleteTokenizer {
    meta: TokenizerMeta,
}

impl Default for DeleteTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: DELETE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 3,
            },
        }
    }
}

impl Tokenizer for DeleteTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for DeleteTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains("~~") {
            return None;
        }

        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut matched = false;

        while let Some(start_offset) = input[cursor..].find("~~") {
            let start = cursor + start_offset;
            let content_start = start + 2;

            let Some(end_offset) = input[content_start..].find("~~") else {
                break;
            };
            let end = content_start + end_offset;
            if end == content_start {
                cursor = end + 2;
                continue;
            }

            if start > cursor {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[cursor..start].to_string(),
                }));
            }

            nodes.push(Node::Delete(DeleteNode {
                position: None,
                children: vec![Node::Text(Text {
                    position: None,
                    value: input[content_start..end].to_string(),
                })],
            }));

            matched = true;
            cursor = end + 2;
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
