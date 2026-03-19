use std::fmt::{Display, Formatter};

use crate::constant::TokenizerType;
use crate::types::match_inline::FindDelimiterGenerator;
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

pub struct FindDelimiterGeneratorBy<F>
where
    F: FnMut(usize, usize) -> Option<TokenDelimiter>,
{
    find_delimiter: F,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl<F> FindDelimiterGeneratorBy<F>
where
    F: FnMut(usize, usize) -> Option<TokenDelimiter>,
{
    pub fn new(find_delimiter: F) -> Self {
        Self {
            find_delimiter,
            last_end_index: None,
            last_delimiter: None,
        }
    }
}

impl<F> FindDelimiterGenerator for FindDelimiterGeneratorBy<F>
where
    F: FnMut(usize, usize) -> Option<TokenDelimiter>,
{
    fn next(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        let (start_index, end_index) = range_index;

        if self.last_end_index == Some(end_index) {
            match self.last_delimiter.as_ref() {
                Some(delimiter) if delimiter.start_index >= start_index => {
                    return Some(delimiter.clone());
                }
                None => return None,
                _ => {}
            }
        }

        self.last_end_index = Some(end_index);
        self.last_delimiter = (self.find_delimiter)(start_index, end_index);
        self.last_delimiter.clone()
    }
}

pub fn gen_find_delimiter<F>(find_delimiter: F) -> FindDelimiterGeneratorBy<F>
where
    F: FnMut(usize, usize) -> Option<TokenDelimiter>,
{
    FindDelimiterGeneratorBy::new(find_delimiter)
}

#[allow(non_snake_case)]
pub fn genFindDelimiter<F>(find_delimiter: F) -> FindDelimiterGeneratorBy<F>
where
    F: FnMut(usize, usize) -> Option<TokenDelimiter>,
{
    gen_find_delimiter(find_delimiter)
}
