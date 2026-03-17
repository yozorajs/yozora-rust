use yozora_ast::{Html, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const HTML_INLINE_TOKENIZER_NAME: &str = "@yozora/tokenizer-html-inline";

#[derive(Debug, Clone)]
pub struct HtmlInlineTokenizer {
    meta: TokenizerMeta,
}

impl Default for HtmlInlineTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: HTML_INLINE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 9,
            },
        }
    }
}

impl Tokenizer for HtmlInlineTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for HtmlInlineTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('<') || !input.contains('>') {
            return None;
        }

        let mut nodes = Vec::new();
        let mut cursor = 0usize;
        let mut last_emit = 0usize;
        let mut matched = false;

        while let Some(start_offset) = input[cursor..].find('<') {
            let start = cursor + start_offset;
            if is_escaped(input, start) {
                cursor = start + 1;
                continue;
            }
            let Some(end) = find_tag_end(input, start) else {
                break;
            };

            let candidate = &input[start..=end];
            if is_angle_link_destination(input, start, end) {
                cursor = start + 1;
                continue;
            }
            if !looks_like_html_tag(candidate) {
                cursor = start + 1;
                continue;
            }

            if start > last_emit {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[last_emit..start].to_string(),
                }));
            }

            nodes.push(Node::Html(Html {
                position: None,
                value: candidate.to_string(),
            }));

            matched = true;
            cursor = end + 1;
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

fn looks_like_html_tag(candidate: &str) -> bool {
    if !(candidate.starts_with('<') && candidate.ends_with('>')) {
        return false;
    }

    if candidate.starts_with("<!--") {
        return is_valid_html_comment(candidate);
    }
    if candidate.starts_with("<![CDATA[") {
        return candidate.ends_with("]]>");
    }
    if candidate.starts_with("<?") {
        return candidate.ends_with("?>");
    }
    if candidate.starts_with("<!") {
        return !candidate.contains('\0');
    }

    let inner = &candidate[1..candidate.len() - 1];
    if inner.is_empty() {
        return false;
    }

    let bytes = inner.as_bytes();

    if bytes[0] == b'/' {
        return is_valid_closing_tag(inner);
    }

    is_valid_opening_tag(inner)
}

fn is_valid_closing_tag(inner: &str) -> bool {
    let bytes = inner.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'/' || !bytes[1].is_ascii_alphabetic() {
        return false;
    }

    let mut i = 2usize;
    while i < bytes.len() && is_tag_name_char(bytes[i]) {
        i += 1;
    }

    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    i == bytes.len()
}

fn is_valid_opening_tag(inner: &str) -> bool {
    let bytes = inner.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_alphabetic() {
        return false;
    }

    let mut i = 1usize;
    while i < bytes.len() && is_tag_name_char(bytes[i]) {
        i += 1;
    }

    if i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' {
        return false;
    }

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            return true;
        }

        if bytes[i] == b'/' {
            return i + 1 == bytes.len();
        }

        if !is_attr_name_start(bytes[i]) {
            return false;
        }

        i += 1;
        while i < bytes.len() && is_attr_name_char(bytes[i]) {
            i += 1;
        }

        let attr_end = i;
        let mut after_attr = i;
        while after_attr < bytes.len() && bytes[after_attr].is_ascii_whitespace() {
            after_attr += 1;
        }

        if after_attr < bytes.len() && bytes[after_attr] == b'=' {
            i = after_attr + 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                return false;
            }

            let quote = bytes[i];
            if quote == b'"' || quote == b'\'' {
                i += 1;
                let mut found_closing_quote = false;
                while i < bytes.len() {
                    if bytes[i] == quote {
                        found_closing_quote = true;
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                if !found_closing_quote {
                    return false;
                }
            } else {
                let value_start = i;
                while i < bytes.len() && is_unquoted_attr_value_char(bytes[i]) {
                    i += 1;
                }
                if i == value_start {
                    return false;
                }
            }
        } else {
            i = attr_end;
        }

        if i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' {
            return false;
        }
    }

    true
}

fn is_valid_html_comment(candidate: &str) -> bool {
    if candidate.len() < 7 || !candidate.starts_with("<!--") || !candidate.ends_with("-->") {
        return false;
    }

    let body = &candidate[4..candidate.len() - 3];
    !body.contains("--") && !body.ends_with('-')
}

fn is_attr_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b':'
}

fn is_attr_name_char(b: u8) -> bool {
    is_attr_name_start(b) || b.is_ascii_digit() || b == b'.' || b == b'-'
}

fn is_unquoted_attr_value_char(b: u8) -> bool {
    !b.is_ascii_whitespace() && !matches!(b, b'"' | b'\'' | b'=' | b'<' | b'>' | b'`')
}

fn find_tag_end(input: &str, start: usize) -> Option<usize> {
    if input.as_bytes().get(start).copied()? != b'<' {
        return None;
    }

    let rest = &input[start..];
    if rest.starts_with("<!--") {
        return rest.find("-->").map(|idx| start + idx + 2);
    }
    if rest.starts_with("<![CDATA[") {
        return rest.find("]]>").map(|idx| start + idx + 2);
    }
    if rest.starts_with("<?") {
        return rest.find("?>").map(|idx| start + idx + 1);
    }

    let bytes = input.as_bytes();
    let mut i = start + 1;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }

        if b == b'"' || b == b'\'' {
            quote = Some(b);
            i += 1;
            continue;
        }

        if b == b'>' {
            return Some(i);
        }

        i += 1;
    }

    None
}

fn is_tag_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-'
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

fn is_angle_link_destination(input: &str, start: usize, end: usize) -> bool {
    let bytes = input.as_bytes();
    if start < 2 || bytes.get(start).copied() != Some(b'<') {
        return false;
    }
    if bytes.get(start - 1).copied() != Some(b'(') || bytes.get(start - 2).copied() != Some(b']') {
        return false;
    }
    if bytes.get(end).copied() != Some(b'>') {
        return false;
    }
    if bytes[start + 1..end]
        .iter()
        .any(|b| *b == b'\n' || *b == b'\r')
    {
        return false;
    }

    let mut i = end + 1;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    bytes.get(i).copied() == Some(b')')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_parse_multiline_tag_with_quoted_inner_angle_brackets() {
        let tokenizer = HtmlInlineTokenizer::default();
        let input = "<a foo=\"bar\" bam = 'baz <em>\"</em>'\n_boolean zoop:33=zoop:33 />";
        let end = find_tag_end(input, 0).expect("should find end");
        assert_eq!(end, input.len() - 1);
        let inner = &input[1..input.len() - 1];
        assert!(is_valid_opening_tag(inner));
        assert!(looks_like_html_tag(input));
        let nodes = tokenizer.tokenize_inline(input, None).expect("should match");
        assert_eq!(nodes.len(), 1);
        let Node::Html(node) = &nodes[0] else {
            panic!("expected html node");
        };
        assert_eq!(node.value, input);
    }

    #[test]
    fn should_not_parse_invalid_tag_whitespace() {
        let tokenizer = HtmlInlineTokenizer::default();
        let input = "<bar/ >";
        let inner = &input[1..input.len() - 1];
        assert!(!is_valid_opening_tag(inner));
        assert!(!looks_like_html_tag(input));
        assert!(tokenizer.tokenize_inline(input, None).is_none());
    }
}
