use yozora_ast::{ImageReference, Node, ReferenceType, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const IMAGE_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-image-reference";

#[derive(Debug, Clone)]
pub struct ImageReferenceTokenizer {
    meta: TokenizerMeta,
}

impl Default for ImageReferenceTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: IMAGE_REFERENCE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 6,
            },
        }
    }
}

impl Tokenizer for ImageReferenceTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for ImageReferenceTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains("![") {
            return None;
        }

        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut last_emit = 0usize;
        let mut matched = false;

        while let Some(offset) = input[cursor..].find("![") {
            let start = cursor + offset;
            if is_escaped(input, start) {
                cursor = start + 2;
                continue;
            }

            let Some(label_end_offset) = input[start + 2..].find(']') else {
                break;
            };
            let label_end = start + 2 + label_end_offset;
            let label = input[start + 2..label_end].trim();
            if label.is_empty() {
                cursor = start + 2;
                continue;
            }

            if input[label_end + 1..].starts_with('(') {
                cursor = start + 2;
                continue;
            }

            let (identifier, label_for_node, reference_type, consumed_len) =
                if input[label_end + 1..].starts_with("[]") {
                    (
                        label.to_string(),
                        label.to_string(),
                        ReferenceType::Collapsed,
                        label_end + 3 - start,
                    )
                } else if input[label_end + 1..].starts_with('[') {
                    let Some(ref_end_offset) = input[label_end + 2..].find(']') else {
                        cursor = start + 2;
                        continue;
                    };
                    let ref_end = label_end + 2 + ref_end_offset;
                    let reference = input[label_end + 2..ref_end].trim();
                    if reference.is_empty() {
                        cursor = start + 2;
                        continue;
                    }
                    (
                        reference.to_string(),
                        reference.to_string(),
                        ReferenceType::Full,
                        ref_end + 1 - start,
                    )
                } else {
                    (
                        label.to_string(),
                        label.to_string(),
                        ReferenceType::Shortcut,
                        label_end + 1 - start,
                    )
                };

            if start > last_emit {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[last_emit..start].to_string(),
                }));
            }

            nodes.push(Node::ImageReference(ImageReference {
                position: None,
                identifier: normalize_identifier(&identifier),
                label: label_for_node,
                reference_type,
                alt: normalize_alt_text(label),
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

fn normalize_identifier(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}

fn normalize_alt_text(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }

        if matches!(ch, '*' | '_' | '`') {
            continue;
        }

        out.push(ch);
    }

    out
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
