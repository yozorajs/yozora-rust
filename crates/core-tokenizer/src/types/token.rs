use std::any::Any;
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use yozora_ast::{NodeType, Position};

use crate::constant::DelimiterType;

pub type TokenData = Arc<dyn Any + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDataTypeMismatch {
    pub expected: &'static str,
}

impl Display for TokenDataTypeMismatch {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "token data is not `{}`", self.expected)
    }
}

impl std::error::Error for TokenDataTypeMismatch {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedBlockTokenError {
    DataTypeMismatch(TokenDataTypeMismatch),
    MissingPosition,
}

impl Display for TypedBlockTokenError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataTypeMismatch(error) => Display::fmt(error, formatter),
            Self::MissingPosition => formatter.write_str("block token position is missing"),
        }
    }
}

impl std::error::Error for TypedBlockTokenError {}

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

pub type PartialInlineToken = InlineToken;

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

#[derive(Debug, Clone)]
pub struct TypedInlineToken<T> {
    pub tokenizer: Arc<str>,
    pub node_type: NodeType,
    pub start_index: usize,
    pub end_index: usize,
    pub children: Arc<Vec<InlineToken>>,
    data: Arc<T>,
}

impl<T> TypedInlineToken<T> {
    pub fn data(&self) -> &T {
        &self.data
    }
}

impl<T> Deref for TypedInlineToken<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> TryFrom<&InlineToken> for TypedInlineToken<T>
where
    T: Any + Send + Sync + 'static,
{
    type Error = TokenDataTypeMismatch;

    fn try_from(token: &InlineToken) -> Result<Self, Self::Error> {
        let data = Arc::clone(&token.data)
            .downcast::<T>()
            .map_err(|_| TokenDataTypeMismatch {
                expected: std::any::type_name::<T>(),
            })?;
        Ok(Self {
            tokenizer: Arc::clone(&token.tokenizer),
            node_type: token.node_type,
            start_index: token.start_index,
            end_index: token.end_index,
            children: Arc::clone(&token.children),
            data,
        })
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

pub type PartialBlockToken = BlockToken;

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

#[derive(Debug, Clone)]
pub struct TypedBlockToken<T> {
    pub tokenizer: Arc<str>,
    pub node_type: NodeType,
    pub position: Position,
    pub children: BlockTokenChildren,
    data: Arc<T>,
}

impl<T> TypedBlockToken<T> {
    pub fn data(&self) -> &T {
        &self.data
    }
}

impl<T> Deref for TypedBlockToken<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> TryFrom<&BlockToken> for TypedBlockToken<T>
where
    T: Any + Send + Sync + 'static,
{
    type Error = TypedBlockTokenError;

    fn try_from(token: &BlockToken) -> Result<Self, Self::Error> {
        let data = Arc::clone(&token.data).downcast::<T>().map_err(|_| {
            TypedBlockTokenError::DataTypeMismatch(TokenDataTypeMismatch {
                expected: std::any::type_name::<T>(),
            })
        })?;
        Ok(Self {
            tokenizer: Arc::clone(&token.tokenizer),
            node_type: token.node_type,
            position: token
                .position
                .clone()
                .ok_or(TypedBlockTokenError::MissingPosition)?,
            children: token.children.clone(),
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use yozora_ast::{Point, Position};

    use super::{BlockToken, InlineToken, TypedBlockToken, TypedBlockTokenError, TypedInlineToken};

    #[derive(Debug)]
    struct TestData {
        value: usize,
    }

    #[test]
    fn typed_tokens_expose_specific_data() {
        let inline = InlineToken::new("inline", "text", (1, 2)).with_data(TestData { value: 3 });
        let point = Point {
            line: 1,
            column: 1,
            offset: Some(0),
        };
        let block = BlockToken::new(
            "block",
            "paragraph",
            Some(Position {
                start: point,
                end: point,
                indent: None,
            }),
        )
        .with_data(TestData { value: 4 });

        let typed_inline = TypedInlineToken::<TestData>::try_from(&inline).unwrap();
        let typed_block = TypedBlockToken::<TestData>::try_from(&block).unwrap();

        assert_eq!(typed_inline.start_index, 1);
        assert_eq!(typed_inline.value, 3);
        assert_eq!(typed_block.value, 4);

        let missing = BlockToken::new("block", "paragraph", None).with_data(TestData { value: 5 });
        assert_eq!(
            TypedBlockToken::<TestData>::try_from(&missing).unwrap_err(),
            TypedBlockTokenError::MissingPosition
        );
    }

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
