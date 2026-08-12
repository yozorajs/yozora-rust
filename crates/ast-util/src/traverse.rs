use yozora_ast::{Node, Root};

use crate::NodeMatcher;

pub enum ParentRef<'a> {
    Root(&'a Root),
    Node(&'a Node),
}

impl ParentRef<'_> {
    pub fn children(&self) -> &[Node] {
        match self {
            Self::Root(root) => &root.children,
            Self::Node(node) => node.children().unwrap_or(&[]),
        }
    }
}

pub fn traverse_ast<F>(root: &Root, matcher: NodeMatcher<'_>, mut touch: F)
where
    F: FnMut(&Node, ParentRef<'_>, usize),
{
    enum Frame<'a> {
        Root(&'a Root, usize),
        Node(&'a Node, usize),
    }

    let mut stack = vec![Frame::Root(root, 0)];
    while let Some(frame) = stack.last_mut() {
        let (children, index) = match frame {
            Frame::Root(root, index) => (&root.children[..], index),
            Frame::Node(node, index) => (node.children().unwrap_or(&[]), index),
        };
        if *index >= children.len() {
            stack.pop();
            continue;
        }

        let child_index = *index;
        *index += 1;
        let child = &children[child_index];
        if matcher.matches(child) {
            let parent = match frame {
                Frame::Root(root, _) => ParentRef::Root(root),
                Frame::Node(node, _) => ParentRef::Node(node),
            };
            touch(child, parent, child_index);
        }
        if child
            .children()
            .is_some_and(|children| !children.is_empty())
        {
            stack.push(Frame::Node(child, 0));
        }
    }
}
