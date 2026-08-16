use yozora_core_tokenizer::PhrasingContentLine;

#[derive(Debug, Clone)]
pub struct SetextHeadingTokenData {
    pub marker: i32,
    pub lines: Vec<PhrasingContentLine>,
}

pub const SETEXT_HEADING_TOKENIZER_NAME: &str = "@yozora/tokenizer-setext-heading";
