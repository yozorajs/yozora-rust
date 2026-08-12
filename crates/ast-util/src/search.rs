use yozora_ast::{Node, Root};

use crate::ParentRef;

pub fn search_node<F>(root: &Root, mut is_target: F) -> Option<Vec<usize>>
where
    F: FnMut(&Node, ParentRef<'_>, usize) -> bool,
{
    enum Frame<'a> {
        Root(&'a Root, usize),
        Node(&'a Node, usize),
    }

    let mut path = Vec::new();
    let mut stack = vec![Frame::Root(root, 0)];
    while !stack.is_empty() {
        let depth = stack.len();
        let frame = stack.last_mut().expect("search stack should contain frame");
        let (children, index) = match frame {
            Frame::Root(root, index) => (&root.children[..], index),
            Frame::Node(node, index) => (node.children().unwrap_or(&[]), index),
        };
        if *index >= children.len() {
            stack.pop();
            path.pop();
            continue;
        }
        let child_index = *index;
        *index += 1;
        let child = &children[child_index];
        if path.len() < depth {
            path.push(child_index);
        } else {
            path[depth - 1] = child_index;
        }
        let parent = match frame {
            Frame::Root(root, _) => ParentRef::Root(root),
            Frame::Node(node, _) => ParentRef::Node(node),
        };
        if is_target(child, parent, child_index) {
            path.truncate(depth);
            return Some(path);
        }
        if child
            .children()
            .is_some_and(|children| !children.is_empty())
        {
            stack.push(Frame::Node(child, 0));
        }
    }
    None
}
