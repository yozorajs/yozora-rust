use std::any::Any;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use yozora_ast::{NodeType, Position};

use crate::constant::DelimiterType;

pub type TokenData = Arc<dyn Any + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub node_type: NodeType,
    pub start_index: usize,
    pub end_index: usize,
    pub children: Arc<Vec<InlineToken>>,
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
            node_type,
            start_index: interval.0,
            end_index: interval.1,
            children: Arc::new(Vec::new()),
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

    pub fn with_children(mut self, children: Vec<InlineToken>) -> Self {
        self.children = Arc::new(children);
        self
    }

    pub fn data_as<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.data.as_ref().downcast_ref::<T>()
    }
}

impl Drop for InlineToken {
    fn drop(&mut self) {
        let mut stack = match Arc::try_unwrap(std::mem::take(&mut self.children)) {
            Ok(children) => children,
            Err(children) => {
                drop(children);
                return;
            }
        };
        while let Some(mut token) = stack.pop() {
            if let Ok(children) = Arc::try_unwrap(std::mem::take(&mut token.children)) {
                stack.extend(children);
            }
        }
    }
}

/// Shares immutable block snapshots in O(1) while preserving independent mutation through COW.
#[derive(Debug, Default)]
pub struct BlockTokenChildren(Arc<Vec<BlockToken>>);

impl Clone for BlockTokenChildren {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl From<Vec<BlockToken>> for BlockTokenChildren {
    fn from(children: Vec<BlockToken>) -> Self {
        Self(Arc::new(children))
    }
}

impl BlockTokenChildren {
    pub fn into_vec(self) -> Vec<BlockToken> {
        Arc::unwrap_or_clone(self.0)
    }
}

impl Deref for BlockTokenChildren {
    type Target = Vec<BlockToken>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl DerefMut for BlockTokenChildren {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

impl IntoIterator for BlockTokenChildren {
    type Item = BlockToken;
    type IntoIter = std::vec::IntoIter<BlockToken>;

    fn into_iter(self) -> Self::IntoIter {
        Arc::unwrap_or_clone(self.0).into_iter()
    }
}

impl<'a> IntoIterator for &'a BlockTokenChildren {
    type Item = &'a BlockToken;
    type IntoIter = std::slice::Iter<'a, BlockToken>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut BlockTokenChildren {
    type Item = &'a mut BlockToken;
    type IntoIter = std::slice::IterMut<'a, BlockToken>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[derive(Debug, Clone)]
pub struct BlockToken {
    pub tokenizer: Arc<str>,
    pub node_type: NodeType,
    pub position: Option<Position>,
    pub children: BlockTokenChildren,
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
            node_type,
            position,
            children: BlockTokenChildren::default(),
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

impl Drop for BlockToken {
    fn drop(&mut self) {
        let mut stack = match Arc::try_unwrap(std::mem::take(&mut self.children).0) {
            Ok(children) => children,
            Err(children) => {
                drop(children);
                return;
            }
        };
        while let Some(mut token) = stack.pop() {
            if let Ok(children) = Arc::try_unwrap(std::mem::take(&mut token.children).0) {
                stack.extend(children);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BlockToken;

    #[test]
    fn block_token_clone_uses_copy_on_write_children() {
        let mut token = BlockToken::new("root", "root", None);
        token
            .children
            .push(BlockToken::new("paragraph", "paragraph", None));

        let mut cloned = token.clone();
        cloned
            .children
            .push(BlockToken::new("paragraph", "paragraph", None));

        assert_eq!(token.children.len(), 1);
        assert_eq!(cloned.children.len(), 2);
    }

    #[test]
    fn block_token_drop_is_stack_safe() {
        let mut token = BlockToken::new("root", "root", None);
        for _ in 0..20_000 {
            let mut parent = BlockToken::new("blockquote", "blockquote", None);
            parent.children.push(token);
            token = parent;
        }
        drop(token);
    }
}
