use yozora_core_tokenizer::{DelimiterType, TokenDelimiter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutolinkExtensionContentType {
    Uri,
    UriWww,
    Email,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutolinkExtensionDelimiter {
    pub delimiter_type: DelimiterType,
    pub start_index: usize,
    pub end_index: usize,
    pub thickness: usize,
    pub original_thickness: usize,
    pub content_type: AutolinkExtensionContentType,
}

impl AutolinkExtensionDelimiter {
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
pub struct AutolinkExtensionTokenData {
    pub content_type: AutolinkExtensionContentType,
}

pub const AUTOLINK_EXTENSION_TOKENIZER_NAME: &str = "@yozora/tokenizer-autolink-extension";
