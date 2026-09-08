use yozora_ast::Node;

use crate::types::parse_inline::ParseInlinePhaseApi;
use crate::types::token::InlineToken;

pub type ParseInlineHookCreator<'a> =
    dyn Fn(&'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> + 'a;

pub trait ParseInlineHook {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node>;
}

pub enum ParseInlineGeneratorResult<'a> {
    Yield(&'a [InlineToken]),
    Complete(Vec<Node>),
}

pub trait ParseInlineGenerator<'a> {
    /// Start with `None`, then resume each yield with its parsed child nodes.
    fn resume(&mut self, children: Option<Vec<Node>>) -> ParseInlineGeneratorResult<'a>;
}

pub enum ParseInlineHookResult<'a> {
    Nodes(Vec<Node>),
    Generator(Box<dyn ParseInlineGenerator<'a> + 'a>),
}

impl ParseInlineHookResult<'_> {
    /// Compatibility path for direct callers of a hook's synchronous `parse`.
    /// The processor drives generators on an explicit stack instead.
    pub fn resolve(self, api: &dyn ParseInlinePhaseApi) -> Vec<Node> {
        let mut generator = match self {
            Self::Nodes(nodes) => return nodes,
            Self::Generator(generator) => generator,
        };
        let mut children = None;
        loop {
            match generator.resume(children.take()) {
                ParseInlineGeneratorResult::Yield(tokens) => {
                    children = Some(api.parse_inline_tokens(Some(tokens)));
                }
                ParseInlineGeneratorResult::Complete(nodes) => return nodes,
            }
        }
    }
}
