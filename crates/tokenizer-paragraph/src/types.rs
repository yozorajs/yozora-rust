use yozora_core_tokenizer::PhrasingContentLine;

#[derive(Debug, Clone)]
pub struct ParagraphTokenData {
    pub lines: Vec<PhrasingContentLine>,
}

pub const PARAGRAPH_TOKENIZER_NAME: &str = "@yozora/tokenizer-paragraph";
