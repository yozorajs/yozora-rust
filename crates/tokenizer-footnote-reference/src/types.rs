pub type FootnoteReferenceDelimiter = yozora_core_tokenizer::TokenDelimiter;

#[derive(Debug, Clone)]
pub struct FootnoteReferenceTokenData {
    pub identifier: String,
    pub label: String,
}

pub const FOOTNOTE_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-reference";
