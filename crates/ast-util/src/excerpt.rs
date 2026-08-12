use yozora_ast::{Node, Root};

use crate::tree::{clone_with_children, clone_with_literal_value, literal_value};
use crate::{shallow_clone_ast, ParentRef};

pub fn calc_excerpt_ast(root: &Root, prune_length: usize) -> Root {
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

    let mut remaining = prune_length;
    let mut stack = vec![Frame {
        parent: Parent::Root(root),
        children: &root.children,
        output: Vec::new(),
        index: 0,
    }];
    let mut terminated = remaining == 0;
    loop {
        let frame = stack.last_mut().expect("excerpt stack should not be empty");
        if !terminated && frame.index < frame.children.len() {
            let node = &frame.children[frame.index];
            frame.index += 1;
            if let Some(value) = literal_value(node) {
                let utf16_length = value.encode_utf16().count();
                if utf16_length > remaining {
                    let value = value
                        .chars()
                        .scan(0usize, |width, character| {
                            let next = *width + character.len_utf16();
                            if next > remaining {
                                None
                            } else {
                                *width = next;
                                Some(character)
                            }
                        })
                        .collect::<String>();
                    if !value.is_empty() {
                        frame.output.push(clone_with_literal_value(node, value));
                    }
                    remaining = 0;
                    terminated = true;
                } else {
                    frame.output.push(node.clone());
                    remaining -= utf16_length;
                    terminated = remaining == 0;
                }
            } else if let Some(children) = node.children() {
                stack.push(Frame {
                    parent: Parent::Node(node),
                    children,
                    output: Vec::new(),
                    index: 0,
                });
            } else {
                frame.output.push(node.clone());
            }
            continue;
        }

        let frame = stack.pop().expect("excerpt frame should exist");
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

pub fn get_excerpt_ast(root: &Root, prune_length: usize, excerpt_separator: Option<&str>) -> Root {
    if let Some(separator) = excerpt_separator
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let excerpt = shallow_clone_ast(root, |node, _: ParentRef<'_>, _| {
            literal_value(node).is_some_and(|value| value.trim() == separator)
        });
        if excerpt.children != root.children {
            return excerpt;
        }
    }
    calc_excerpt_ast(root, prune_length)
}
