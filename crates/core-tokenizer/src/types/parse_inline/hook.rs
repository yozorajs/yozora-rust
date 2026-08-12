use yozora_ast::Node;

use crate::types::parse_inline::ParseInlinePhaseApi;
use crate::types::token::InlineToken;

pub type ParseInlineHookCreator<'a> =
    dyn Fn(&'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> + 'a;

pub trait ParseInlineHook {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node>;
}
