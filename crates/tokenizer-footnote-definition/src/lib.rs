use yozora_ast::{FootnoteDefinition, Node, Text};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const FOOTNOTE_DEFINITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-definition";

#[derive(Debug, Clone)]
pub struct FootnoteDefinitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for FootnoteDefinitionTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FOOTNOTE_DEFINITION_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 9,
            },
        }
    }
}

impl Tokenizer for FootnoteDefinitionTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for FootnoteDefinitionTokenizer {
    fn can_interrupt_paragraph(&self) -> bool {
        false
    }

    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        if count_leading_spaces(first) >= 4 {
            return None;
        }

        let trimmed = first.trim_start();
        let (label, body_start) = parse_footnote_label_and_colon(trimmed)?;

        let mut consumed_lines = 1usize;
        let mut body_lines = Vec::<String>::new();
        let first_body = trimmed[body_start..].trim_start();
        let mut can_lazy_continue = false;

        if !first_body.is_empty() {
            body_lines.push(first_body.to_string());
            can_lazy_continue = true;
        }

        for line in lines.iter().skip(1) {
            if line.trim().is_empty() {
                if !body_lines.is_empty() {
                    body_lines.push(String::new());
                }
                consumed_lines += 1;
                can_lazy_continue = false;
                continue;
            }

            if let Some(stripped) = strip_indented_prefix(line) {
                body_lines.push(stripped.to_string());
                consumed_lines += 1;
                can_lazy_continue = true;
                continue;
            }

            if can_lazy_continue && !is_footnote_definition_opener(line) {
                body_lines.push(line.trim_end().to_string());
                consumed_lines += 1;
                continue;
            }

            break;
        }

        Some(BlockTokenizeResult {
            node: Node::FootnoteDefinition(FootnoteDefinition {
                position: None,
                identifier: normalize_identifier(label),
                label: label.to_string(),
                children: vec![Node::Text(Text {
                    position: None,
                    value: body_lines.join("\n"),
                })],
            }),
            consumed_lines,
        })
    }
}

fn count_leading_spaces(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

fn parse_footnote_label_and_colon(input: &str) -> Option<(&str, usize)> {
    if !input.starts_with("[^") {
        return None;
    }

    let bytes = input.as_bytes();
    let mut i = 2usize;
    let mut has_non_whitespace = false;

    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 >= bytes.len() {
                    return None;
                }
                has_non_whitespace = true;
                i += 2;
            }
            b'[' => return None,
            b']' => {
                if !has_non_whitespace || i + 1 >= bytes.len() || bytes[i + 1] != b':' {
                    return None;
                }

                let label = input[2..i].trim();
                if label.is_empty() {
                    return None;
                }
                return Some((label, i + 2));
            }
            ch => {
                if !(ch as char).is_whitespace() {
                    has_non_whitespace = true;
                }
                i += 1;
            }
        }
    }

    None
}

fn strip_indented_prefix(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix("    ") {
        return Some(rest);
    }
    line.strip_prefix('\t')
}

fn is_footnote_definition_opener(line: &str) -> bool {
    let trimmed = line.trim_start();
    parse_footnote_label_and_colon(trimmed).is_some()
}

fn normalize_identifier(label: &str) -> String {
    normalize_whitespace(label).to_lowercase()
}

fn normalize_whitespace(label: &str) -> String {
    let mut out = String::new();
    for (idx, chunk) in label.split_whitespace().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(chunk);
    }
    out
}
