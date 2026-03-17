use yozora_ast::{Link, Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const AUTOLINK_TOKENIZER_NAME: &str = "@yozora/tokenizer-autolink";

#[derive(Debug, Clone)]
pub struct AutolinkTokenizer {
    meta: TokenizerMeta,
}

impl Default for AutolinkTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: AUTOLINK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 9,
            },
        }
    }
}

impl Tokenizer for AutolinkTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for AutolinkTokenizer {
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
            let Some(end_offset) = input[start + 1..].find('>') else {
                break;
            };
            let end = start + 1 + end_offset;

            let candidate = &input[start + 1..end];
            let Some(url) = normalize_autolink_url(candidate) else {
                cursor = start + 1;
                continue;
            };

            if start > last_emit {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[last_emit..start].to_string(),
                }));
            }

            nodes.push(Node::Link(Link {
                position: None,
                url,
                title: None,
                children: vec![Node::Text(Text {
                    position: None,
                    value: candidate.to_string(),
                })],
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

fn normalize_autolink_url(candidate: &str) -> Option<String> {
    if candidate.is_empty() || candidate.chars().any(char::is_whitespace) {
        return None;
    }

    if is_absolute_uri(candidate) {
        return Some(encode_link_destination(candidate));
    }

    if is_email_like(candidate) {
        return Some(format!("mailto:{candidate}"));
    }

    None
}

fn encode_link_destination(destination: &str) -> String {
    let mut decoded = destination.to_string();
    loop {
        let Ok(next) = try_percent_decode_once(&decoded) else {
            break;
        };

        if next == decoded {
            break;
        }

        decoded = next;
    }

    encode_uri_like(&decoded)
}

fn try_percent_decode_once(input: &str) -> Result<String, ()> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(());
            }

            let hi = from_hex(bytes[i + 1]).ok_or(())?;
            let lo = from_hex(bytes[i + 2]).ok_or(())?;
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).map_err(|_| ())
}

fn from_hex(ch: u8) -> Option<u8> {
    match ch {
        b'0'..=b'9' => Some(ch - b'0'),
        b'a'..=b'f' => Some(ch - b'a' + 10),
        b'A'..=b'F' => Some(ch - b'A' + 10),
        _ => None,
    }
}

fn encode_uri_like(input: &str) -> String {
    let mut out = String::new();
    for &b in input.as_bytes() {
        if is_allowed_uri_byte(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(hex_digit((b >> 4) & 0x0f));
            out.push(hex_digit(b & 0x0f));
        }
    }
    out
}

fn is_allowed_uri_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b';' | b','
                | b'/'
                | b'?'
                | b':'
                | b'@'
                | b'&'
                | b'='
                | b'+'
                | b'$'
                | b'-'
                | b'_'
                | b'.'
                | b'!'
                | b'~'
                | b'*'
                | b'\''
                | b'('
                | b')'
                | b'#'
        )
}

fn hex_digit(v: u8) -> char {
    match v {
        0..=9 => (b'0' + v) as char,
        10..=15 => (b'A' + (v - 10)) as char,
        _ => '0',
    }
}

fn is_absolute_uri(candidate: &str) -> bool {
    let Some((scheme, _rest)) = candidate.split_once(':') else {
        return false;
    };
    if scheme.len() < 2 || scheme.len() > 32 {
        return false;
    }

    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '-' || ch == '.')
}

fn is_email_like(candidate: &str) -> bool {
    let Some((local, domain)) = candidate.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    if !domain.contains('.') {
        return false;
    }

    if !local
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '+' | '-' | '_'))
    {
        return false;
    }

    domain
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
}
