use yozora_ast::ReferenceType;
use yozora_core_tokenizer::{MatchInlinePhaseApi, NodeInterval};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageReferenceToken {
    Text(NodeInterval),
    ImageReference {
        interval: NodeInterval,
        identifier: String,
        label: String,
        reference_type: ReferenceType,
        alt: String,
    },
}

pub(crate) fn match_image_reference_tokens(
    input: &str,
    match_api: Option<&dyn MatchInlinePhaseApi>,
) -> Option<Vec<ImageReferenceToken>> {
    if !input.contains("![") {
        return None;
    }

    let mut tokens = Vec::new();
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

        let (identifier_raw, label_for_node, reference_type, consumed_len) =
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

        let identifier = normalize_identifier(&identifier_raw);
        if match_api.is_some_and(|api| !api.has_definition(&identifier)) {
            cursor = start + 2;
            continue;
        }

        if start > last_emit {
            tokens.push(ImageReferenceToken::Text(NodeInterval {
                start_index: last_emit,
                end_index: start,
            }));
        }

        tokens.push(ImageReferenceToken::ImageReference {
            interval: NodeInterval {
                start_index: start,
                end_index: start + consumed_len,
            },
            identifier,
            label: label_for_node,
            reference_type,
            alt: normalize_alt_text(label),
        });

        matched = true;
        cursor = start + consumed_len;
        last_emit = cursor;
    }

    if !matched {
        return None;
    }

    if last_emit < input.len() {
        tokens.push(ImageReferenceToken::Text(NodeInterval {
            start_index: last_emit,
            end_index: input.len(),
        }));
    }

    Some(tokens)
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
