use yozora_ast::Node;

use crate::types::token::BlockToken;

pub trait ParseBlockHook {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node>;
}
