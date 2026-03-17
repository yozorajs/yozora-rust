use yozora_ast::{Node, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const SOFT_BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-soft-break";

#[derive(Debug, Clone)]
pub struct SoftBreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for SoftBreakTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: SOFT_BREAK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 0,
            },
        }
    }
}

impl Tokenizer for SoftBreakTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for SoftBreakTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('\n') {
            return None;
        }

        let mut out = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let mut i = 0usize;
        let mut changed = false;

        while i < bytes.len() {
            if bytes[i] != b'\n' {
                out.push(bytes[i] as char);
                i += 1;
                continue;
            }

            while out.ends_with(' ') || out.ends_with('\t') {
                out.pop();
                changed = true;
            }

            out.push('\n');
            i += 1;

            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
                changed = true;
            }
        }

        if !changed {
            return None;
        }

        Some(vec![Node::Text(Text {
            position: None,
            value: out,
        })])
    }
}
