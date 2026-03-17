use yozora_ast::{Node, Text};
use yozora_character::{calc_escaped_string_from_node_points, create_node_point_generator};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const BACKSLASH_ESCAPE_TOKENIZER_NAME: &str = "@yozora/tokenizer-backslash-escape";

#[derive(Debug, Clone)]
pub struct BackslashEscapeTokenizer {
    meta: TokenizerMeta,
}

impl Default for BackslashEscapeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: BACKSLASH_ESCAPE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 0,
            },
        }
    }
}

impl Tokenizer for BackslashEscapeTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for BackslashEscapeTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('\\') && !input.contains('&') {
            return None;
        }

        let chunks = create_node_point_generator(input);
        let Some(points) = chunks.first() else {
            return None;
        };

        let decoded = calc_escaped_string_from_node_points(points, 0, points.len(), false);
        if decoded == input {
            return None;
        }

        Some(vec![Node::Text(Text {
            position: None,
            value: decoded,
        })])
    }
}
