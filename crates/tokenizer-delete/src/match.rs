use yozora_core_tokenizer::NodeInterval;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeleteToken {
    Text(NodeInterval),
    Delete {
        interval: NodeInterval,
        content: String,
    },
}

pub(crate) fn match_delete_tokens(input: &str) -> Option<Vec<DeleteToken>> {
    if !input.contains("~~") {
        return None;
    }

    let mut tokens = Vec::new();
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
            tokens.push(DeleteToken::Text(NodeInterval {
                start_index: cursor,
                end_index: start,
            }));
        }

        tokens.push(DeleteToken::Delete {
            interval: NodeInterval {
                start_index: start,
                end_index: end + 2,
            },
            content: input[content_start..end].to_string(),
        });

        matched = true;
        cursor = end + 2;
    }

    if !matched {
        return None;
    }

    if cursor < input.len() {
        tokens.push(DeleteToken::Text(NodeInterval {
            start_index: cursor,
            end_index: input.len(),
        }));
    }

    Some(tokens)
}
