use std::fmt::{Display, Formatter};

use crate::engine::constant::TokenizerType;
use crate::engine::tokenizer::EngineTokenizer;

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

impl EngineTokenizer for BaseInlineTokenizer {
    fn tokenizer_type(&self) -> TokenizerType {
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
