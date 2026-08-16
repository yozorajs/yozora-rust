use yozora_character::NodePoint;
use yozora_core_tokenizer::{DelimiterType, ResultOfRequiredEater, TokenDelimiter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutolinkContentType {
    Uri,
    Email,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutolinkDelimiter {
    pub delimiter_type: DelimiterType,
    pub start_index: usize,
    pub end_index: usize,
    pub thickness: usize,
    pub original_thickness: usize,
    pub content_type: AutolinkContentType,
}

impl AutolinkDelimiter {
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
pub struct AutolinkTokenData {
    pub content_type: AutolinkContentType,
}

pub type AutolinkContentEater = fn(&[NodePoint], usize, usize) -> ResultOfRequiredEater;

#[derive(Debug, Clone, Copy)]
pub struct AutolinkContentHelper {
    pub content_type: AutolinkContentType,
    pub eat: AutolinkContentEater,
}

pub const AUTOLINK_TOKENIZER_NAME: &str = "@yozora/tokenizer-autolink";
