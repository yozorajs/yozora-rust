use yozora_core_tokenizer::{BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const FENCED_BLOCK_TOKENIZER_NAME: &str = "@yozora/tokenizer-fenced-block";

#[derive(Debug, Clone)]
pub struct FencedBlockTokenizer {
    meta: TokenizerMeta,
}

impl Default for FencedBlockTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FENCED_BLOCK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 0,
            },
        }
    }
}

impl Tokenizer for FencedBlockTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for FencedBlockTokenizer {
    fn tokenize_block(
        &self,
        _input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<yozora_ast::Node> {
        None
    }
}
