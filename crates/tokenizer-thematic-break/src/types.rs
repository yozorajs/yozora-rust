#[derive(Debug, Clone)]
pub struct ThematicBreakTokenData {
    pub marker: i32,
    pub continuous: bool,
}

pub const THEMATIC_BREAK_TOKENIZER_NAME: &str = "@yozora/tokenizer-thematic-break";
