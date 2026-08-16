use yozora_core_tokenizer::PhrasingContentLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HtmlBlockConditionType {
    Condition1 = 1,
    Condition2 = 2,
    Condition3 = 3,
    Condition4 = 4,
    Condition5 = 5,
    Condition6 = 6,
    Condition7 = 7,
}

#[derive(Debug, Clone)]
pub struct HtmlBlockTokenData {
    pub condition: HtmlBlockConditionType,
    pub lines: Vec<PhrasingContentLine>,
}

pub const HTML_BLOCK_TOKENIZER_NAME: &str = "@yozora/tokenizer-html-block";
