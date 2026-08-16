use yozora_ast::ReferenceType;
use yozora_core_tokenizer::{DelimiterType, InlineToken, TokenDelimiter};

#[derive(Debug, Clone)]
pub struct LinkReferenceTokenData {
    pub identifier: String,
    pub label: String,
    pub reference_type: ReferenceType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkReferenceDelimiterBracket {
    pub start_index: usize,
    pub end_index: usize,
    pub label: Option<String>,
    pub identifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkReferenceDelimiter {
    pub delimiter_type: DelimiterType,
    pub start_index: usize,
    pub end_index: usize,
    pub thickness: usize,
    pub original_thickness: usize,
    pub brackets: Vec<LinkReferenceDelimiterBracket>,
}

impl LinkReferenceDelimiter {
    pub fn to_core(&self) -> TokenDelimiter {
        TokenDelimiter {
            delimiter_type: self.delimiter_type,
            start_index: self.start_index,
            end_index: self.end_index,
            thickness: self.thickness,
            original_thickness: self.original_thickness,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinkReferenceProcessDelimiterPairResult {
    pub tokens: Vec<InlineToken>,
    pub remain_opener_delimiter: Option<LinkReferenceDelimiter>,
    pub remain_closer_delimiter: Option<LinkReferenceDelimiter>,
}

pub const LINK_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-link-reference";
