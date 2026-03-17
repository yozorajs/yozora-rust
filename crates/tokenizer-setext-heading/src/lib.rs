use yozora_ast::{Heading, Node, Text};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const SETEXT_HEADING_TOKENIZER_NAME: &str = "@yozora/tokenizer-setext-heading";

#[derive(Debug, Clone)]
pub struct SetextHeadingTokenizer {
    meta: TokenizerMeta,
}

impl Default for SetextHeadingTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: SETEXT_HEADING_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 5,
            },
        }
    }
}

impl Tokenizer for SetextHeadingTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for SetextHeadingTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        if lines.len() < 2 {
            return None;
        }

        if leading_space_count(lines[0]) >= 4 {
            return None;
        }

        let mut content_lines = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if index == 0 {
                if line.trim().is_empty() {
                    return None;
                }
                content_lines.push(trim_line_end(line));
                continue;
            }

            if let Some(depth) = parse_underline_depth(line) {
                let content = content_lines.join("\n");
                let content = content.trim().to_string();
                if content.is_empty() {
                    return None;
                }

                return Some(BlockTokenizeResult {
                    node: Node::Heading(Heading {
                        position: None,
                        identifier: None,
                        depth,
                        children: vec![Node::Text(Text {
                            position: None,
                            value: content,
                        })],
                    }),
                    consumed_lines: index + 1,
                });
            }

            if line.trim().is_empty() || leading_space_count(line) >= 4 {
                break;
            }

            content_lines.push(trim_line_end(line));
        }

        None
    }
}

fn parse_underline_depth(line: &str) -> Option<u8> {
    if leading_space_count(line) >= 4 {
        return None;
    }

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let marker = trimmed.chars().next()?;
    if marker != '=' && marker != '-' {
        return None;
    }

    if !trimmed.chars().all(|ch| ch == marker) {
        return None;
    }

    Some(if marker == '=' { 1 } else { 2 })
}

fn trim_line_end(line: &str) -> String {
    line.trim_end().to_string()
}

fn leading_space_count(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}
