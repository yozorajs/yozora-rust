use yozora_ast::{Node, Root};

use crate::{NodeMatcher, ParentRef};

use super::{create_postorder_mutation, MutationStep, NodeReplacement};

pub fn shallow_mutate_ast_in_postorder<F>(
    root: &Root,
    matcher: NodeMatcher<'_>,
    mut replace: F,
) -> Root
where
    F: FnMut(&Node, ParentRef<'_>, usize) -> NodeReplacement,
{
    let mut mutation = create_postorder_mutation(root, matcher);
    let mut replacement = None;
    loop {
        match mutation.next(replacement.take()) {
            MutationStep::Request(request) => {
                replacement = Some(replace(request.node, request.parent, request.child_index));
            }
            MutationStep::Complete(root) => return root,
        }
    }
}
