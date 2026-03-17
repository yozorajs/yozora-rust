use yozora_ast::{Node, Paragraph};
use yozora_core_tokenizer::{
    BlockFallbackTokenizer, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const PARAGRAPH_TOKENIZER_NAME: &str = "@yozora/tokenizer-paragraph";

#[derive(Debug, Clone)]
pub struct ParagraphTokenizer {
    meta: TokenizerMeta,
}

impl Default for ParagraphTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: PARAGRAPH_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: -1,
            },
        }
    }
}

impl Tokenizer for ParagraphTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for ParagraphTokenizer {}

impl BlockFallbackTokenizer for ParagraphTokenizer {
    fn build_block(
        &self,
        inline_children: Vec<Node>,
        position: Option<yozora_ast::Position>,
    ) -> Node {
        Node::Paragraph(Paragraph {
            position,
            children: inline_children,
        })
    }
}
