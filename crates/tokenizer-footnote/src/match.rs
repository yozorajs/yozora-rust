use yozora_core_tokenizer::NodeInterval;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FootnoteToken {
    Text(NodeInterval),
    Literal(String),
    Footnote {
        interval: NodeInterval,
        content: String,
    },
}

pub(crate) fn match_footnote_tokens(input: &str) -> Option<Vec<FootnoteToken>> {
    if !input.contains("^[") {
        return None;
    }

    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    let mut matched = false;

    while let Some(offset) = input[cursor..].find("^[") {
        let start = cursor + offset;
        if is_escaped(input, start) {
            let escape_start = start.saturating_sub(1);
            if escape_start > cursor {
                tokens.push(FootnoteToken::Text(NodeInterval {
                    start_index: cursor,
                    end_index: escape_start,
                }));
            }

            tokens.push(FootnoteToken::Literal("^[".to_string()));

            matched = true;
            cursor = start + 2;
            continue;
        }

        let content_start = start + 2;
        let Some(end) = find_matching_bracket(input, content_start) else {
            break;
        };

        if start > cursor {
            tokens.push(FootnoteToken::Text(NodeInterval {
                start_index: cursor,
                end_index: start,
            }));
        }

        tokens.push(FootnoteToken::Footnote {
            interval: NodeInterval {
                start_index: start,
                end_index: end + 1,
            },
            content: input[content_start..end].to_string(),
        });

        matched = true;
        cursor = end + 1;
    }

    if !matched {
        return None;
    }

    if cursor < input.len() {
        tokens.push(FootnoteToken::Text(NodeInterval {
            start_index: cursor,
            end_index: input.len(),
        }));
    }

    Some(tokens)
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
