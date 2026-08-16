pub type InlineMathDelimiter = yozora_core_tokenizer::TokenDelimiter;

#[derive(Debug, Clone)]
pub struct InlineMathTokenData {
    pub thickness: usize,
}

#[derive(Debug, Clone)]
pub struct InlineMathTokenizerOptions {
    pub name: Option<String>,
    pub priority: Option<i32>,
    pub backtick_required: bool,
}

impl Default for InlineMathTokenizerOptions {
    fn default() -> Self {
        Self {
            name: None,
            priority: None,
            backtick_required: true,
        }
    }
}

pub const INLINE_MATH_TOKENIZER_NAME: &str = "@yozora/tokenizer-inline-math";
pub const INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME: &str =
    "@yozora/tokenizer-inline-math_with_backtick";
pub const INLINE_MATH_TOKENIZER_NAME_WITH_BACKTICK: &str = INLINE_MATH_WITH_BACKTICK_TOKENIZER_NAME;
