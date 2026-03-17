use yozora_ast::{Code, Node};
use yozora_character::{calc_escaped_string_from_node_points, create_node_point_generator};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const FENCED_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-fenced-code";

#[derive(Debug, Clone)]
pub struct FencedCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for FencedCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FENCED_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 10,
            },
        }
    }
}

impl Tokenizer for FencedCodeTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for FencedCodeTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        let opening = parse_opening_fence(first)?;

        let mut content_lines = Vec::new();
        let mut consumed_lines = lines.len();

        for (index, line) in lines.iter().enumerate().skip(1) {
            if is_closing_fence(line, opening.marker, opening.fence_len) {
                consumed_lines = index + 1;
                break;
            }

            content_lines.push(strip_opening_indent(line, opening.indent).to_string());
        }

        let value = if content_lines.is_empty() {
            "\n".to_string()
        } else {
            format!("{}\n", content_lines.join("\n"))
        };

        Some(BlockTokenizeResult {
            node: Node::Code(Code {
                position: None,
                value,
                lang: opening.lang,
                meta: opening.meta,
            }),
            consumed_lines,
        })
    }
}

#[derive(Debug, Clone)]
struct OpeningFence {
    marker: char,
    fence_len: usize,
    indent: usize,
    lang: Option<String>,
    meta: Option<String>,
}

fn parse_opening_fence(line: &str) -> Option<OpeningFence> {
    let indent = line.chars().take_while(|ch| *ch == ' ').count();
    if indent >= 4 {
        return None;
    }

    let remainder = &line[indent..];
    let marker = remainder.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }

    let fence_len = remainder.chars().take_while(|ch| *ch == marker).count();
    if fence_len < 3 {
        return None;
    }

    let info = remainder[fence_len..].trim();
    if marker == '`' && info.contains('`') {
        return None;
    }

    let (lang, meta) = parse_info_string(info);
    Some(OpeningFence {
        marker,
        fence_len,
        indent,
        lang,
        meta,
    })
}

fn parse_info_string(info: &str) -> (Option<String>, Option<String>) {
    if info.is_empty() {
        return (None, None);
    }

    let mut chunks = info.splitn(2, char::is_whitespace);
    let lang = chunks
        .next()
        .map(decode_escaped_content)
        .filter(|v| !v.is_empty());
    let meta = chunks
        .next()
        .map(str::trim)
        .map(decode_escaped_content)
        .filter(|v| !v.is_empty());
    (lang, meta)
}

fn decode_escaped_content(input: &str) -> String {
    let chunks = create_node_point_generator(input);
    let Some(points) = chunks.first() else {
        return String::new();
    };

    calc_escaped_string_from_node_points(points, 0, points.len(), false)
}

fn is_closing_fence(line: &str, marker: char, min_len: usize) -> bool {
    let indent = line.chars().take_while(|ch| *ch == ' ').count();
    if indent >= 4 {
        return false;
    }

    let remainder = &line[indent..];
    let fence_len = remainder.chars().take_while(|ch| *ch == marker).count();
    if fence_len < min_len {
        return false;
    }

    remainder[fence_len..].trim().is_empty()
}

fn strip_opening_indent<'a>(line: &'a str, indent: usize) -> &'a str {
    let mut rest = line;
    let mut removed = 0usize;
    while removed < indent {
        let Some(next) = rest.strip_prefix(' ') else {
            break;
        };
        rest = next;
        removed += 1;
    }
    rest
}
