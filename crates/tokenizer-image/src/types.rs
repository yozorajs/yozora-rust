use yozora_core_tokenizer::{DelimiterType, NodeInterval, TokenDelimiter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageDelimiter {
    pub delimiter_type: DelimiterType,
    pub start_index: usize,
    pub end_index: usize,
    pub thickness: usize,
    pub original_thickness: usize,
    pub destination_content: Option<NodeInterval>,
    pub title_content: Option<NodeInterval>,
}

impl ImageDelimiter {
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
pub struct ImageTokenData {
    pub destination_content: Option<NodeInterval>,
    pub title_content: Option<NodeInterval>,
}

pub const IMAGE_TOKENIZER_NAME: &str = "@yozora/tokenizer-image";
