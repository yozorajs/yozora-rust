use yozora_ast::{Node, Text};
use yozora_core_tokenizer::{
    InlineFallbackTokenizer, InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const TEXT_TOKENIZER_NAME: &str = "@yozora/tokenizer-text";

#[derive(Debug, Clone)]
pub struct TextTokenizer {
    meta: TokenizerMeta,
}

impl Default for TextTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: TEXT_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: -1,
            },
        }
    }
}

impl Tokenizer for TextTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for TextTokenizer {}

impl InlineFallbackTokenizer for TextTokenizer {
    fn build_inline(&self, value: &str, position: Option<yozora_ast::Position>) -> Node {
        Node::Text(Text {
            position,
            value: value.to_string(),
        })
    }
}
