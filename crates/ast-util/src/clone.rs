use yozora_ast::{Node, Root};

use crate::tree::clone_with_children;
use crate::ParentRef;

pub fn shallow_clone_ast<F>(root: &Root, mut end_condition: F) -> Root
where
    F: FnMut(&Node, ParentRef<'_>, usize) -> bool,
{
    enum Parent<'a> {
        Root(&'a Root),
        Node(&'a Node),
    }
    struct Frame<'a> {
        parent: Parent<'a>,
        children: &'a [Node],
        next_children: Vec<Node>,
        index: usize,
    }

    let mut stack = vec![Frame {
        parent: Parent::Root(root),
        children: &root.children,
        next_children: Vec::new(),
        index: 0,
    }];
    let mut terminated = false;
    loop {
        let Some(frame) = stack.last_mut() else {
            return root.clone();
        };
        if !terminated && frame.index < frame.children.len() {
            let child_index = frame.index;
            frame.index += 1;
            let child = &frame.children[child_index];
            let parent = match frame.parent {
                Parent::Root(root) => ParentRef::Root(root),
                Parent::Node(node) => ParentRef::Node(node),
            };
            if end_condition(child, parent, child_index) {
                terminated = true;
            } else if let Some(children) = child.children() {
                stack.push(Frame {
                    parent: Parent::Node(child),
                    children,
                    next_children: Vec::new(),
                    index: 0,
                });
            } else {
                frame.next_children.push(child.clone());
            }
            continue;
        }

        let frame = stack.pop().expect("clone stack should contain frame");
        let next_node = match frame.parent {
            Parent::Root(root) => {
                return Root {
                    node_type: root.node_type.clone(),
                    position: root.position.clone(),
                    children: frame.next_children,
                };
            }
            Parent::Node(node) => clone_with_children(node, frame.next_children),
        };
        stack
            .last_mut()
            .expect("node frame should have parent")
            .next_children
            .push(next_node);
    }
}
