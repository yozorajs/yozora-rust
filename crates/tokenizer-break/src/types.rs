#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakTokenMarkerType {
    Backslash,
    MoreThanTwoSpaces,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakDelimiter {
    pub delimiter_type: yozora_core_tokenizer::DelimiterType,
    pub start_index: usize,
    pub end_index: usize,
    pub thickness: usize,
    pub original_thickness: usize,
    pub marker_type: BreakTokenMarkerType,
}

impl BreakDelimiter {
    pub fn to_core(&self) -> yozora_core_tokenizer::TokenDelimiter {
        yozora_core_tokenizer::TokenDelimiter {
            delimiter_type: self.delimiter_type,
            start_index: self.start_index,
            end_index: self.end_index,
            thickness: self.thickness,
            original_thickness: self.original_thickness,
        }
    }
}

pub const BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-break";
