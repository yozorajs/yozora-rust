use yozora_core_tokenizer::NodeInterval;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BreakToken {
    Text(NodeInterval),
    Break(NodeInterval),
}

pub(crate) fn match_break_tokens(input: &str) -> Option<Vec<BreakToken>> {
    if !input.contains('\n') {
        return None;
    }

    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
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
            tokens.push(BreakToken::Text(NodeInterval {
                start_index: cursor,
                end_index: marker_start,
            }));
        }

        tokens.push(BreakToken::Break(NodeInterval {
            start_index: marker_start,
            end_index: index + 1,
        }));

        matched = true;
        cursor = index;
        index += 1;
    }

    if !matched {
        return None;
    }

    if cursor < input.len() {
        tokens.push(BreakToken::Text(NodeInterval {
            start_index: cursor,
            end_index: input.len(),
        }));
    }

    Some(tokens)
}
