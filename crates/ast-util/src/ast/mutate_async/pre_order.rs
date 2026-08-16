use yozora_ast::{Node, Root};

use crate::tree::clone_with_children;
use crate::{NodeMatcher, NodeReplacementFuture, ParentRef};

use super::append_replacement;

pub async fn shallow_mutate_ast_in_preorder_async<F>(
    root: &Root,
    matcher: NodeMatcher<'_>,
    mut replace: F,
) -> Root
where
    F: for<'a> FnMut(&'a Node, ParentRef<'a>, usize) -> NodeReplacementFuture<'a>,
{
    enum Parent<'a> {
        Root(&'a Root),
        Node(&'a Node),
    }
    struct Frame<'a> {
        parent: Parent<'a>,
        children: &'a [Node],
        output: Vec<Node>,
        index: usize,
    }
    let mut stack = vec![Frame {
        parent: Parent::Root(root),
        children: &root.children,
        output: Vec::new(),
        index: 0,
    }];
    loop {
        let frame = stack
            .last_mut()
            .expect("mutation stack should not be empty");
        if frame.index < frame.children.len() {
            let child_index = frame.index;
            frame.index += 1;
            let child = &frame.children[child_index];
            let parent = match frame.parent {
                Parent::Root(root) => ParentRef::Root(root),
                Parent::Node(node) => ParentRef::Node(node),
            };
            if matcher.matches(child) {
                append_replacement(replace(child, parent, child_index).await, &mut frame.output);
            } else if let Some(children) = child.children().filter(|value| !value.is_empty()) {
                stack.push(Frame {
                    parent: Parent::Node(child),
                    children,
                    output: Vec::new(),
                    index: 0,
                });
            } else {
                frame.output.push(child.clone());
            }
            continue;
        }
        let frame = stack.pop().expect("mutation frame should exist");
        match frame.parent {
            Parent::Root(root) => {
                return Root {
                    node_type: root.node_type.clone(),
                    position: root.position.clone(),
                    children: frame.output,
                }
            }
            Parent::Node(node) => stack
                .last_mut()
                .expect("node frame should have parent")
                .output
                .push(clone_with_children(node, frame.output)),
        }
    }
}
