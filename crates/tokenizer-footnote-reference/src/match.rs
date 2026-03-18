use yozora_core_tokenizer::{MatchInlinePhaseApi, NodeInterval};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FootnoteReferenceToken {
    Text(NodeInterval),
    Reference {
        interval: NodeInterval,
        identifier: String,
        label: String,
    },
}

pub(crate) fn match_footnote_reference_tokens(
    input: &str,
    match_api: Option<&dyn MatchInlinePhaseApi>,
) -> Option<Vec<FootnoteReferenceToken>> {
    if !input.contains("[^") {
        return None;
    }

    let mut tokens = Vec::new();
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

        let identifier = normalize_identifier(label);
        if match_api.is_some_and(|ctx| !ctx.has_footnote_definition(&identifier)) {
            cursor = start + 2;
            continue;
        }

        if start > cursor {
            tokens.push(FootnoteReferenceToken::Text(NodeInterval {
                start_index: cursor,
                end_index: start,
            }));
        }

        tokens.push(FootnoteReferenceToken::Reference {
            interval: NodeInterval {
                start_index: start,
                end_index: end + 1,
            },
            identifier,
            label: label.to_string(),
        });

        matched = true;
        cursor = end + 1;
    }

    if !matched {
        return None;
    }

    if cursor < input.len() {
        tokens.push(FootnoteReferenceToken::Text(NodeInterval {
            start_index: cursor,
            end_index: input.len(),
        }));
    }

    Some(tokens)
}

fn normalize_identifier(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}
