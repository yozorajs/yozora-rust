use std::any::Any;
use std::sync::Arc;

use yozora_ast::{NodeType, Position};

use crate::constant::DelimiterType;

pub type TokenData = Arc<dyn Any + Send + Sync + 'static>;
pub type TokenizerId = u64;
pub const UNKNOWN_TOKENIZER_ID: TokenizerId = u64::MAX;

#[derive(Debug, Clone)]
pub struct TokenDelimiter {
    pub delimiter_type: DelimiterType,
    pub start_index: usize,
    pub end_index: usize,
    pub thickness: usize,
    pub original_thickness: usize,
}

#[derive(Debug, Clone)]
pub struct InlineToken {
    pub tokenizer: Arc<str>,
    pub tokenizer_id: TokenizerId,
    pub node_type: NodeType,
    pub start_index: usize,
    pub end_index: usize,
    pub data: TokenData,
}

impl InlineToken {
    pub fn new(
        tokenizer: impl Into<Arc<str>>,
        node_type: NodeType,
        interval: (usize, usize),
    ) -> Self {
        Self {
            tokenizer: tokenizer.into(),
            tokenizer_id: UNKNOWN_TOKENIZER_ID,
            node_type,
            start_index: interval.0,
            end_index: interval.1,
            data: Arc::new(()),
        }
    }

    pub fn with_data<T>(mut self, data: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.data = Arc::new(data);
        self
    }

    pub fn data_as<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.data.as_ref().downcast_ref::<T>()
    }
}

#[derive(Debug, Clone)]
pub struct BlockToken {
    pub tokenizer: Arc<str>,
    pub tokenizer_id: TokenizerId,
    pub node_type: NodeType,
    pub position: Option<Position>,
    pub children: Vec<BlockToken>,
    pub data: TokenData,
}

impl BlockToken {
    pub fn new(
        tokenizer: impl Into<Arc<str>>,
        node_type: NodeType,
        position: Option<Position>,
    ) -> Self {
        Self {
            tokenizer: tokenizer.into(),
            tokenizer_id: UNKNOWN_TOKENIZER_ID,
            node_type,
            position,
            children: Vec::new(),
            data: Arc::new(()),
        }
    }

    pub fn with_data<T>(mut self, data: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.data = Arc::new(data);
        self
    }

    pub fn data_as<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.data.as_ref().downcast_ref::<T>()
    }
}
