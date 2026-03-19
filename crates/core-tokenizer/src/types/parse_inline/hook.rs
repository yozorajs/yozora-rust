use yozora_ast::Node;

use crate::types::token::InlineToken;

pub trait ParseInlineHook {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node>;
}
