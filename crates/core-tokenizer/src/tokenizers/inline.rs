use std::fmt::{Display, Formatter};

use crate::constant::TokenizerType;
use crate::types::token::TokenDelimiter;
use crate::types::tokenizer::Tokenizer;

#[derive(Debug, Clone)]
pub struct BaseInlineTokenizer {
    pub name: String,
    pub priority: i32,
}

impl BaseInlineTokenizer {
    pub fn new(name: impl Into<String>, priority: i32) -> Self {
        Self {
            name: name.into(),
            priority,
        }
    }
}

impl Tokenizer for BaseInlineTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Inline
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

impl Display for BaseInlineTokenizer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

pub fn gen_find_delimiter<F>(
    range_index: (usize, usize),
    last_end_index: &mut Option<usize>,
    last_delimiter: &mut Option<TokenDelimiter>,
    mut find_delimiter: F,
) -> Option<TokenDelimiter>
where
    F: FnMut(usize, usize) -> Option<TokenDelimiter>,
{
    let (start_index, end_index) = range_index;

    if *last_end_index == Some(end_index) {
        match last_delimiter.as_ref() {
            Some(delimiter) if delimiter.start_index >= start_index => {
                return Some(delimiter.clone());
            }
            None => return None,
            _ => {}
        }
    }

    *last_end_index = Some(end_index);
    *last_delimiter = find_delimiter(start_index, end_index);
    last_delimiter.clone()
}

#[allow(non_snake_case)]
pub fn genFindDelimiter<F>(
    range_index: (usize, usize),
    last_end_index: &mut Option<usize>,
    last_delimiter: &mut Option<TokenDelimiter>,
    find_delimiter: F,
) -> Option<TokenDelimiter>
where
    F: FnMut(usize, usize) -> Option<TokenDelimiter>,
{
    gen_find_delimiter(range_index, last_end_index, last_delimiter, find_delimiter)
}
