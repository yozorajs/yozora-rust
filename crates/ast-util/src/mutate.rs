use yozora_ast::{Node, Root};

use crate::tree::clone_with_children;
use crate::{NodeMatcher, ParentRef};

pub enum NodeReplacement {
    One(Node),
    Many(Vec<Node>),
    Remove,
}

impl NodeReplacement {
    fn append_to(self, output: &mut Vec<Node>) {
        match self {
            Self::One(node) => output.push(node),
            Self::Many(nodes) => output.extend(nodes),
            Self::Remove => {}
        }
    }
}

pub fn shallow_mutate_ast_in_preorder<F>(
    root: &Root,
    matcher: NodeMatcher<'_>,
    mut replace: F,
) -> Root
where
    F: FnMut(&Node, ParentRef<'_>, usize) -> NodeReplacement,
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
                replace(child, parent, child_index).append_to(&mut frame.output);
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
                };
            }
            Parent::Node(node) => stack
                .last_mut()
                .expect("node frame should have parent")
                .output
                .push(clone_with_children(node, frame.output)),
        }
    }
}

pub fn shallow_mutate_ast_in_postorder<F>(
    root: &Root,
    matcher: NodeMatcher<'_>,
    mut replace: F,
) -> Root
where
    F: FnMut(&Node, ParentRef<'_>, usize) -> NodeReplacement,
{
    enum Parent<'a> {
        Root(&'a Root),
        Node(&'a Node),
    }
    struct Frame<'a> {
        parent: Parent<'a>,
        children: &'a [Node],
        traversed: Vec<Node>,
        output: Vec<Node>,
        child_index: usize,
        replace_index: usize,
    }

    let mut stack = vec![Frame {
        parent: Parent::Root(root),
        children: &root.children,
        traversed: Vec::new(),
        output: Vec::new(),
        child_index: 0,
        replace_index: 0,
    }];
    loop {
        let frame = stack
            .last_mut()
            .expect("mutation stack should not be empty");
        if frame.child_index < frame.children.len() {
            let child = &frame.children[frame.child_index];
            frame.child_index += 1;
            if let Some(children) = child.children().filter(|value| !value.is_empty()) {
                stack.push(Frame {
                    parent: Parent::Node(child),
                    children,
                    traversed: Vec::new(),
                    output: Vec::new(),
                    child_index: 0,
                    replace_index: 0,
                });
            } else {
                frame.traversed.push(child.clone());
            }
            continue;
        }

        if frame.replace_index < frame.traversed.len() {
            let index = frame.replace_index;
            frame.replace_index += 1;
            let child = &frame.traversed[index];
            if matcher.matches(child) {
                let parent = match frame.parent {
                    Parent::Root(root) => ParentRef::Root(root),
                    Parent::Node(node) => ParentRef::Node(node),
                };
                replace(child, parent, index).append_to(&mut frame.output);
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
                };
            }
            Parent::Node(node) => stack
                .last_mut()
                .expect("node frame should have parent")
                .traversed
                .push(clone_with_children(node, frame.output)),
        }
    }
}

pub fn mutate_node<F>(node: &mut Node, visitor: &mut F)
where
    F: FnMut(&mut Node),
{
    visitor(node);
    match node {
        Node::Admonition(node) => {
            for child in &mut node.title {
                mutate_node(child, visitor);
            }
            for child in &mut node.children {
                mutate_node(child, visitor);
            }
        }
        Node::Blockquote(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Delete(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Emphasis(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Footnote(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::FootnoteDefinition(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Heading(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Link(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::LinkReference(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::List(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::ListItem(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Paragraph(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Strong(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::Table(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::TableRow(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        Node::TableCell(node) => node
            .children
            .iter_mut()
            .for_each(|node| mutate_node(node, visitor)),
        _ => {}
    }
}

pub fn mutate_root<F>(root: &mut Root, visitor: &mut F)
where
    F: FnMut(&mut Node),
{
    root.children
        .iter_mut()
        .for_each(|node| mutate_node(node, visitor));
}
