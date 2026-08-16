pub type InlineCodeDelimiter = yozora_core_tokenizer::TokenDelimiter;

#[derive(Debug, Clone)]
pub struct InlineCodeTokenData {
    pub thickness: usize,
}

pub const INLINE_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-inline-code";
