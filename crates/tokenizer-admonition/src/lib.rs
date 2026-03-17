use yozora_ast::{Admonition, Node, Text};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const ADMONITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-admonition";

#[derive(Debug, Clone)]
pub struct AdmonitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for AdmonitionTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: ADMONITION_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 12,
            },
        }
    }
}

impl Tokenizer for AdmonitionTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for AdmonitionTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        let (keyword, title) = parse_opening_admonition(first)?;

        let mut consumed_lines = 1usize;
        let mut body_lines = Vec::new();
        let mut has_closing = false;
        for line in lines.iter().skip(1) {
            if line.trim() == ":::" {
                consumed_lines += 1;
                has_closing = true;
                break;
            }
            body_lines.push((*line).to_string());
            consumed_lines += 1;
        }

        if !has_closing {
            return None;
        }

        let title_nodes = if title.is_empty() {
            Vec::new()
        } else {
            vec![Node::Text(Text {
                position: None,
                value: title,
            })]
        };

        let children = if body_lines.is_empty() {
            Vec::new()
        } else {
            vec![Node::Text(Text {
                position: None,
                value: body_lines.join("\n"),
            })]
        };

        Some(BlockTokenizeResult {
            node: Node::Admonition(Admonition {
                position: None,
                keyword,
                title: title_nodes,
                children,
            }),
            consumed_lines,
        })
    }
}

fn parse_opening_admonition(line: &str) -> Option<(String, String)> {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return None;
    }

    let trimmed = line[leading_spaces..].trim_end();
    if !trimmed.starts_with(":::") {
        return None;
    }

    let rest = trimmed[3..].trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let keyword = parts.next()?.trim();
    if keyword.is_empty() {
        return None;
    }
    let title = parts.next().map(str::trim).unwrap_or_default().to_string();
    Some((keyword.to_string(), title))
}
