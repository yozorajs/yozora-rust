use std::fmt::{Display, Formatter};

use crate::engine::constant::TokenizerType;
use crate::engine::tokenizer::EngineTokenizer;

#[derive(Debug, Clone)]
pub struct BaseBlockTokenizer {
    pub name: String,
    pub priority: i32,
}

impl BaseBlockTokenizer {
    pub fn new(name: impl Into<String>, priority: i32) -> Self {
        Self {
            name: name.into(),
            priority,
        }
    }
}

impl EngineTokenizer for BaseBlockTokenizer {
    fn tokenizer_type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

impl Display for BaseBlockTokenizer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}
