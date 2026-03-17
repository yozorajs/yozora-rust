use yozora_ast::{Math, Node};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const MATH_TOKENIZER_NAME: &str = "@yozora/tokenizer-math";

#[derive(Debug, Clone)]
pub struct MathTokenizer {
    meta: TokenizerMeta,
}

impl Default for MathTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: MATH_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 11,
            },
        }
    }
}

impl Tokenizer for MathTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for MathTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        let trimmed = first.trim();
        let opening_len = trimmed.chars().take_while(|ch| *ch == '$').count();
        if opening_len < 2 {
            return None;
        }

        // Single-line math fence: opening and closing fence lengths must match.
        if !trimmed.chars().all(|ch| ch == '$') {
            let closing_len = trimmed.chars().rev().take_while(|ch| *ch == '$').count();
            if closing_len != opening_len || trimmed.len() <= opening_len + closing_len {
                return None;
            }

            let content = trimmed[opening_len..trimmed.len() - closing_len].trim();
            if content.is_empty() {
                return None;
            }

            return Some(BlockTokenizeResult {
                node: Node::Math(Math {
                    position: None,
                    value: format!("{}\n", content),
                }),
                consumed_lines: 1,
            });
        }

        let fence = "$".repeat(opening_len);
        let mut content = Vec::new();
        let mut consumed_lines = lines.len();

        for (idx, line) in lines.iter().enumerate().skip(1) {
            if line.trim() == fence {
                consumed_lines = idx + 1;
                break;
            }
            content.push((*line).to_string());
        }

        let value = if content.is_empty() {
            String::new()
        } else {
            format!("{}\n", content.join("\n"))
        };

        Some(BlockTokenizeResult {
            node: Node::Math(Math {
                position: None,
                value,
            }),
            consumed_lines,
        })
    }
}
