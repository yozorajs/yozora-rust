pub type EmphasisDelimiter = yozora_core_tokenizer::TokenDelimiter;

#[derive(Debug, Clone)]
pub struct EmphasisTokenData {
    pub thickness: usize,
}

pub const EMPHASIS_TOKENIZER_NAME: &str = "@yozora/tokenizer-emphasis";
