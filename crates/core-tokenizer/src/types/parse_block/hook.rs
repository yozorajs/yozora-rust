use yozora_ast::Node;

use crate::types::token::BlockToken;

pub enum ParseBlockTaskStep {
    /// Suspend the task until core parses the requested child tokens.
    Request(Vec<BlockToken>),
    /// Finish the tokenizer batch with its parsed nodes.
    Done(Vec<Node>),
}

pub trait ParseBlockTask {
    /// Core passes `None` on the first call and `Some(nodes)` after each request.
    fn resume(&mut self, children: Option<Vec<Node>>) -> ParseBlockTaskStep;
}

pub trait ParseBlockHook {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node>;

    /// Built-in containing tokenizers use an owned task to avoid recursive child parsing.
    fn parse_task(&self, _tokens: &[BlockToken]) -> Option<Box<dyn ParseBlockTask>> {
        None
    }
}
