use yozora_core_tokenizer::PhrasingContentLine;

#[derive(Debug, Clone)]
pub struct IndentedCodeTokenData {
    pub lines: Vec<PhrasingContentLine>,
}

pub const INDENTED_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-indented-code";
