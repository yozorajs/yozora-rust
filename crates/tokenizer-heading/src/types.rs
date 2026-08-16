use yozora_core_tokenizer::PhrasingContentLine;

#[derive(Debug, Clone)]
pub struct HeadingTokenData {
    pub depth: u8,
    pub line: PhrasingContentLine,
}

pub const HEADING_TOKENIZER_NAME: &str = "@yozora/tokenizer-heading";
