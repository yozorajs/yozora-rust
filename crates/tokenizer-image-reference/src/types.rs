use yozora_ast::ReferenceType;
use yozora_core_tokenizer::{DelimiterType, TokenDelimiter};
use yozora_tokenizer_link_reference::LinkReferenceDelimiterBracket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReferenceDelimiter {
    pub delimiter_type: DelimiterType,
    pub start_index: usize,
    pub end_index: usize,
    pub thickness: usize,
    pub original_thickness: usize,
    pub brackets: Vec<LinkReferenceDelimiterBracket>,
}

impl ImageReferenceDelimiter {
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
pub struct ImageReferenceTokenData {
    pub identifier: String,
    pub label: String,
    pub reference_type: ReferenceType,
}

pub const IMAGE_REFERENCE_TOKENIZER_NAME: &str = "@yozora/tokenizer-image-reference";
