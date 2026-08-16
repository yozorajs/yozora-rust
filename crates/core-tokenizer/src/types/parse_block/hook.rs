use std::fmt::{Display, Formatter};

use yozora_ast::Node;

use crate::types::parse_block::{ParseBlockPhaseApi, ParseBlockTokensRequest};
use crate::types::token::BlockToken;

pub type ParseBlockHookCreator<'a> =
    dyn Fn(&'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> + 'a;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBlockError {
    message: String,
}

impl ParseBlockError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for ParseBlockError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseBlockError {}

impl From<&str> for ParseBlockError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl From<String> for ParseBlockError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

pub type ParseBlockResult<T> = Result<T, ParseBlockError>;

pub enum ParseBlockGeneratorResume {
    Initial,
    Nodes(Vec<Node>),
    Error(ParseBlockError),
}

pub enum ParseBlockGeneratorResult<'a> {
    Yield(ParseBlockTokensRequest<'a>),
    Complete(Vec<Node>),
}

pub trait ParseBlockGenerator<'a> {
    fn resume(
        &mut self,
        state: ParseBlockGeneratorResume,
    ) -> ParseBlockResult<ParseBlockGeneratorResult<'a>>;
}

pub enum ParseBlockHookResult<'a> {
    Nodes(Vec<Node>),
    Generator(Box<dyn ParseBlockGenerator<'a> + 'a>),
}

impl From<Vec<Node>> for ParseBlockHookResult<'_> {
    fn from(nodes: Vec<Node>) -> Self {
        Self::Nodes(nodes)
    }
}

pub trait ParseBlockHook {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>>;
}
