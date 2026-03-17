use yozora_ast::{FootnoteReference, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const FOOTNOTE_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-reference";

#[derive(Debug, Clone)]
pub struct FootnoteReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteReferenceTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FOOTNOTE_REFERENCE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 7,
            },
        }
    }
}

impl Tokenizer for FootnoteReferenceTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for FootnoteReferenceTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains("[^") {
            return None;
        }

        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut matched = false;

        while let Some(offset) = input[cursor..].find("[^") {
            let start = cursor + offset;
            let content_start = start + 2;
            let Some(end_offset) = input[content_start..].find(']') else {
                break;
            };
            let end = content_start + end_offset;
            let label = input[content_start..end].trim();
            if label.is_empty() {
                cursor = start + 2;
                continue;
            }

            if start > cursor {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[cursor..start].to_string(),
                }));
            }

            nodes.push(Node::FootnoteReference(FootnoteReference {
                position: None,
                identifier: normalize_identifier(label),
                label: label.to_string(),
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

fn normalize_identifier(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}
