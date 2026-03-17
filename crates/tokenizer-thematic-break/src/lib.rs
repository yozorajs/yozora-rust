use yozora_ast::{Node, ThematicBreak};
use yozora_core_tokenizer::{BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const THEMATIC_BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-thematic-break";

#[derive(Debug, Clone)]
pub struct ThematicBreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for ThematicBreakTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: THEMATIC_BREAK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 7,
            },
        }
    }
}

impl Tokenizer for ThematicBreakTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for ThematicBreakTokenizer {
    fn tokenize_block(&self, input: &str, _position: Option<yozora_ast::Position>) -> Option<Node> {
        if input.contains('\n') {
            return None;
        }

        let leading_spaces = input.chars().take_while(|ch| *ch == ' ').count();
        if leading_spaces >= 4 {
            return None;
        }

        let line = input.trim();
        if line.is_empty() {
            return None;
        }

        let mut marker: Option<char> = None;
        let mut count = 0usize;
        for ch in line.chars() {
            if ch == ' ' || ch == '\t' {
                continue;
            }

            if ch != '-' && ch != '*' && ch != '_' {
                return None;
            }

            if let Some(m) = marker {
                if m != ch {
                    return None;
                }
            } else {
                marker = Some(ch);
            }
            count += 1;
        }

        if count < 3 {
            return None;
        }

        Some(Node::ThematicBreak(ThematicBreak { position: None }))
    }
}
