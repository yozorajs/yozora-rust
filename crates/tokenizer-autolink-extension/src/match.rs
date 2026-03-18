use yozora_core_tokenizer::NodeInterval;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AutolinkExtensionToken {
    Text(NodeInterval),
    Link {
        interval: NodeInterval,
        display: String,
        href: String,
    },
}

pub(crate) fn match_autolink_extension_tokens(input: &str) -> Option<Vec<AutolinkExtensionToken>> {
    if !input.contains("www.")
        && !input.contains("http://")
        && !input.contains("https://")
        && !input.contains('@')
    {
        return None;
    }

    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    let mut matched = false;

    while cursor < input.len() {
        let Some(start) = find_candidate_start(input, cursor) else {
            tokens.push(AutolinkExtensionToken::Text(NodeInterval {
                start_index: cursor,
                end_index: input.len(),
            }));
            break;
        };

        if start > cursor {
            tokens.push(AutolinkExtensionToken::Text(NodeInterval {
                start_index: cursor,
                end_index: start,
            }));
        }

        let end = find_candidate_end(input, start);
        let raw_candidate = &input[start..end];
        if let Some((display, href, suffix)) = parse_autolink_extension(raw_candidate) {
            let candidate_end = end.saturating_sub(suffix.len());
            tokens.push(AutolinkExtensionToken::Link {
                interval: NodeInterval {
                    start_index: start,
                    end_index: candidate_end,
                },
                display,
                href,
            });
            if !suffix.is_empty() {
                tokens.push(AutolinkExtensionToken::Text(NodeInterval {
                    start_index: candidate_end,
                    end_index: end,
                }));
            }
            cursor = end;
            matched = true;
        } else {
            tokens.push(AutolinkExtensionToken::Text(NodeInterval {
                start_index: start,
                end_index: end,
            }));
            cursor = end;
        }
    }

    if !matched {
        return None;
    }

    Some(tokens)
}

fn find_candidate_start(input: &str, from: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = from;
    while index < bytes.len() {
        let boundary =
            index == 0 || bytes[index - 1].is_ascii_whitespace() || bytes[index - 1] == b'(';
        if boundary {
            let rest = &input[index..];
            if rest.starts_with("www.")
                || rest.starts_with("http://")
                || rest.starts_with("https://")
                || looks_like_email_start(rest)
            {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

fn looks_like_email_start(rest: &str) -> bool {
    let mut seen_at = false;
    for ch in rest.chars() {
        if ch.is_ascii_whitespace() || ch == '<' {
            break;
        }
        if ch == '@' {
            seen_at = true;
            break;
        }
        if !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '+' || ch == '-' || ch == '_') {
            break;
        }
    }
    seen_at
}

fn find_candidate_end(input: &str, start: usize) -> usize {
    let bytes = input.as_bytes();
    let mut end = start;
    while end < bytes.len() {
        if bytes[end].is_ascii_whitespace() || bytes[end] == b'<' {
            break;
        }
        end += 1;
    }
    end
}

fn parse_autolink_extension(raw: &str) -> Option<(String, String, String)> {
    let mut cutoff = raw.len();

    while cutoff > 0 {
        let Some(last) = raw[..cutoff].chars().last() else {
            break;
        };
        if !matches!(last, '?' | '!' | '.' | ',' | ':' | '*' | '_' | '~') {
            break;
        }
        cutoff -= last.len_utf8();
    }

    cutoff = trim_unbalanced_trailing_parenthesis(raw, cutoff);
    cutoff = trim_entity_like_suffix(raw, cutoff);

    if cutoff == 0 {
        return None;
    }

    let candidate = &raw[..cutoff];
    let suffix = raw[cutoff..].to_string();

    if candidate.starts_with("http://") || candidate.starts_with("https://") {
        return Some((candidate.to_string(), candidate.to_string(), suffix));
    }

    if let Some(domain_and_path) = candidate.strip_prefix("www.") {
        let domain = domain_and_path.split('/').next().unwrap_or_default();
        if is_valid_domain(domain) {
            return Some((candidate.to_string(), format!("http://{candidate}"), suffix));
        }
        return None;
    }

    if is_valid_email(candidate) {
        if suffix.starts_with('_') {
            return None;
        }
        return Some((candidate.to_string(), format!("mailto:{candidate}"), suffix));
    }

    None
}

fn trim_unbalanced_trailing_parenthesis(value: &str, mut cutoff: usize) -> usize {
    loop {
        if cutoff == 0 || !value[..cutoff].ends_with(')') {
            break;
        }
        let slice = &value[..cutoff];
        let opens = slice.chars().filter(|ch| *ch == '(').count();
        let closes = slice.chars().filter(|ch| *ch == ')').count();
        if closes <= opens {
            break;
        }
        cutoff -= ')'.len_utf8();
    }

    cutoff
}

fn trim_entity_like_suffix(value: &str, cutoff: usize) -> usize {
    let slice = &value[..cutoff];
    if !slice.ends_with(';') {
        return cutoff;
    }

    let Some(amp_index) = slice.rfind('&') else {
        return cutoff;
    };
    if amp_index + 1 >= slice.len() {
        return cutoff;
    }

    let entity_body = &slice[amp_index + 1..slice.len() - 1];
    if !entity_body.is_empty() && entity_body.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return amp_index;
    }

    cutoff
}

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }

    domain.split('.').all(|label| {
        if label.is_empty() {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') || label.ends_with('_') {
            return false;
        }
        label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    if !local
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '+' || ch == '-' || ch == '_')
    {
        return false;
    }
    is_valid_domain(domain)
}
