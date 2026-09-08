use yozora_ast::{drop_nodes, Node};

use super::{ParseInlineGenerator, ParseInlineGeneratorResult, ParseInlineHookResult};
use crate::types::token::InlineToken;

/// Parse each container's metadata before its children, then assemble its node.
/// A rejected token is skipped without visiting its children. Token order and
/// side effects (such as URL formatting) match synchronous depth-first parsing.
pub fn parse_inline_containers<'a, T: 'a>(
    tokens: &'a [InlineToken],
    prepare: impl FnMut(&'a InlineToken) -> Option<T> + 'a,
    finish: impl FnMut(T, Vec<Node>) -> Node + 'a,
) -> ParseInlineHookResult<'a> {
    ParseInlineHookResult::Generator(Box::new(ContainerGenerator {
        tokens: tokens.iter(),
        prepare,
        finish,
        pending: None,
        nodes: Vec::with_capacity(tokens.len()),
    }))
}

struct ContainerGenerator<'a, T, P, F> {
    tokens: std::slice::Iter<'a, InlineToken>,
    prepare: P,
    finish: F,
    pending: Option<T>,
    nodes: Vec<Node>,
}

impl<T, P, F> Drop for ContainerGenerator<'_, T, P, F> {
    fn drop(&mut self) {
        drop_nodes(std::mem::take(&mut self.nodes));
    }
}

impl<'a, T, P, F> ParseInlineGenerator<'a> for ContainerGenerator<'a, T, P, F>
where
    P: FnMut(&'a InlineToken) -> Option<T>,
    F: FnMut(T, Vec<Node>) -> Node,
{
    fn resume(&mut self, children: Option<Vec<Node>>) -> ParseInlineGeneratorResult<'a> {
        if let Some(pending) = self.pending.take() {
            self.nodes.push((self.finish)(
                pending,
                children.expect("inline container resumed without child nodes"),
            ));
        } else {
            assert!(children.is_none(), "inline container resumed unexpectedly");
        }
        for token in self.tokens.by_ref() {
            if let Some(pending) = (self.prepare)(token) {
                self.pending = Some(pending);
                return ParseInlineGeneratorResult::Yield(&token.children);
            }
        }
        ParseInlineGeneratorResult::Complete(std::mem::take(&mut self.nodes))
    }
}
