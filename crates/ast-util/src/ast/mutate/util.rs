use yozora_ast::{Node, Root};

use crate::tree::clone_with_children;
use crate::{NodeMatcher, ParentRef};

pub enum NodeReplacement {
    One(Node),
    Many(Vec<Node>),
    Remove,
}

impl NodeReplacement {
    pub(crate) fn append_to(self, output: &mut Vec<Node>) {
        match self {
            Self::One(node) => output.push(node),
            Self::Many(nodes) => output.extend(nodes),
            Self::Remove => {}
        }
    }
}

pub struct MutationRequest<'a> {
    pub node: &'a Node,
    pub parent: ParentRef<'a>,
    pub child_index: usize,
}

pub enum MutationStep<'a> {
    Request(MutationRequest<'a>),
    Complete(Root),
}

enum Parent<'a> {
    Root(&'a Root),
    Node(&'a Node),
}

impl<'a> Parent<'a> {
    fn as_ref(&self) -> ParentRef<'a> {
        match self {
            Self::Root(root) => ParentRef::Root(root),
            Self::Node(node) => ParentRef::Node(node),
        }
    }
}

struct PreorderFrame<'a> {
    parent: Parent<'a>,
    children: &'a [Node],
    output: Vec<Node>,
    index: usize,
}

pub struct PreorderMutation<'root, 'matcher> {
    matcher: NodeMatcher<'matcher>,
    stack: Vec<PreorderFrame<'root>>,
    waiting_for_replacement: bool,
}

pub fn create_preorder_mutation<'root, 'matcher>(
    root: &'root Root,
    matcher: NodeMatcher<'matcher>,
) -> PreorderMutation<'root, 'matcher> {
    PreorderMutation {
        matcher,
        stack: vec![PreorderFrame {
            parent: Parent::Root(root),
            children: &root.children,
            output: Vec::new(),
            index: 0,
        }],
        waiting_for_replacement: false,
    }
}

impl PreorderMutation<'_, '_> {
    pub fn next(&mut self, replacement: Option<NodeReplacement>) -> MutationStep<'_> {
        if self.waiting_for_replacement {
            replacement.unwrap_or(NodeReplacement::Remove).append_to(
                &mut self
                    .stack
                    .last_mut()
                    .expect("mutation frame should exist")
                    .output,
            );
            self.waiting_for_replacement = false;
        } else {
            assert!(replacement.is_none(), "unexpected preorder replacement");
        }

        loop {
            let frame = self
                .stack
                .last_mut()
                .expect("mutation stack should not be empty");
            if frame.index < frame.children.len() {
                let child_index = frame.index;
                frame.index += 1;
                let child = &frame.children[child_index];
                if self.matcher.matches(child) {
                    self.waiting_for_replacement = true;
                    return MutationStep::Request(MutationRequest {
                        node: child,
                        parent: frame.parent.as_ref(),
                        child_index,
                    });
                }
                if let Some(children) = child.children().filter(|value| !value.is_empty()) {
                    self.stack.push(PreorderFrame {
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

            let frame = self.stack.pop().expect("mutation frame should exist");
            match frame.parent {
                Parent::Root(root) => {
                    return MutationStep::Complete(Root {
                        node_type: root.node_type.clone(),
                        position: root.position.clone(),
                        children: frame.output,
                    });
                }
                Parent::Node(node) => self
                    .stack
                    .last_mut()
                    .expect("node frame should have parent")
                    .output
                    .push(clone_with_children(node, frame.output)),
            }
        }
    }
}

struct PostorderFrame<'a> {
    parent: Parent<'a>,
    children: &'a [Node],
    traversed: Vec<Node>,
    output: Vec<Node>,
    child_index: usize,
    replace_index: usize,
}

pub struct PostorderMutation<'root, 'matcher> {
    matcher: NodeMatcher<'matcher>,
    stack: Vec<PostorderFrame<'root>>,
    pending_node: Option<Node>,
    waiting_for_replacement: bool,
}

pub fn create_postorder_mutation<'root, 'matcher>(
    root: &'root Root,
    matcher: NodeMatcher<'matcher>,
) -> PostorderMutation<'root, 'matcher> {
    PostorderMutation {
        matcher,
        stack: vec![PostorderFrame {
            parent: Parent::Root(root),
            children: &root.children,
            traversed: Vec::new(),
            output: Vec::new(),
            child_index: 0,
            replace_index: 0,
        }],
        pending_node: None,
        waiting_for_replacement: false,
    }
}

impl PostorderMutation<'_, '_> {
    pub fn next(&mut self, replacement: Option<NodeReplacement>) -> MutationStep<'_> {
        if self.waiting_for_replacement {
            self.pending_node.take();
            replacement.unwrap_or(NodeReplacement::Remove).append_to(
                &mut self
                    .stack
                    .last_mut()
                    .expect("mutation frame should exist")
                    .output,
            );
            self.waiting_for_replacement = false;
        } else {
            assert!(replacement.is_none(), "unexpected postorder replacement");
        }

        enum Action<'a> {
            Continue,
            Push(&'a Node, &'a [Node]),
            Request(Node, ParentRef<'a>, usize),
            Settle,
        }

        loop {
            let action = {
                let frame = self
                    .stack
                    .last_mut()
                    .expect("mutation stack should not be empty");
                if frame.child_index < frame.children.len() {
                    let child = &frame.children[frame.child_index];
                    frame.child_index += 1;
                    if let Some(children) = child.children().filter(|value| !value.is_empty()) {
                        Action::Push(child, children)
                    } else {
                        frame.traversed.push(child.clone());
                        Action::Continue
                    }
                } else if frame.replace_index < frame.traversed.len() {
                    let child_index = frame.replace_index;
                    frame.replace_index += 1;
                    let child = frame.traversed[child_index].clone();
                    if self.matcher.matches(&child) {
                        Action::Request(child, frame.parent.as_ref(), child_index)
                    } else {
                        frame.output.push(child);
                        Action::Continue
                    }
                } else {
                    Action::Settle
                }
            };

            match action {
                Action::Continue => {}
                Action::Push(node, children) => self.stack.push(PostorderFrame {
                    parent: Parent::Node(node),
                    children,
                    traversed: Vec::new(),
                    output: Vec::new(),
                    child_index: 0,
                    replace_index: 0,
                }),
                Action::Request(node, parent, child_index) => {
                    self.pending_node = Some(node);
                    self.waiting_for_replacement = true;
                    return MutationStep::Request(MutationRequest {
                        node: self
                            .pending_node
                            .as_ref()
                            .expect("pending node should exist"),
                        parent,
                        child_index,
                    });
                }
                Action::Settle => {
                    let frame = self.stack.pop().expect("mutation frame should exist");
                    match frame.parent {
                        Parent::Root(root) => {
                            return MutationStep::Complete(Root {
                                node_type: root.node_type.clone(),
                                position: root.position.clone(),
                                children: frame.output,
                            });
                        }
                        Parent::Node(node) => self
                            .stack
                            .last_mut()
                            .expect("node frame should have parent")
                            .traversed
                            .push(clone_with_children(node, frame.output)),
                    }
                }
            }
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
