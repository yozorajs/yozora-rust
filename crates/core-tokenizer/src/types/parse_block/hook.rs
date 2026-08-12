use yozora_ast::Node;

use crate::types::parse_block::ParseBlockPhaseApi;
use crate::types::token::BlockToken;

pub type ParseBlockHookCreator<'a> =
    dyn Fn(&'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> + 'a;

pub trait ParseBlockHook {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node>;
}
