use yozora_ast::{Node, Root};

use crate::tree::{clone_with_children, clone_with_title, set_position};

pub fn remove_positions(root: &Root) -> Root {
    enum Field {
        Title,
        Children,
    }
    struct Frame<'a> {
        node: &'a Node,
        fields: Vec<(Field, &'a [Node])>,
        field_index: usize,
        child_index: usize,
        output: Vec<Node>,
        title: Option<Vec<Node>>,
        children: Option<Vec<Node>>,
    }

    fn create_frame(node: &Node) -> Frame<'_> {
        let mut fields = Vec::new();
        if let Node::Admonition(admonition) = node {
            fields.push((Field::Title, admonition.title.as_slice()));
        }
        if let Some(children) = node.children() {
            fields.push((Field::Children, children));
        }
        Frame {
            node,
            fields,
            field_index: 0,
            child_index: 0,
            output: Vec::new(),
            title: None,
            children: None,
        }
    }

    let mut root_output = Vec::new();
    let mut root_index = 0usize;
    let mut stack: Vec<Frame<'_>> = Vec::new();
    loop {
        if let Some(frame) = stack.last_mut() {
            if frame.field_index < frame.fields.len() {
                let nodes = frame.fields[frame.field_index].1;
                if frame.child_index < nodes.len() {
                    let child = &nodes[frame.child_index];
                    frame.child_index += 1;
                    stack.push(create_frame(child));
                    continue;
                }
                let output = std::mem::take(&mut frame.output);
                match frame.fields[frame.field_index].0 {
                    Field::Title => frame.title = Some(output),
                    Field::Children => frame.children = Some(output),
                }
                frame.field_index += 1;
                frame.child_index = 0;
                continue;
            }

            let frame = stack.pop().expect("position frame should exist");
            let mut node = frame.children.map_or_else(
                || frame.node.clone(),
                |children| clone_with_children(frame.node, children),
            );
            if let Some(title) = frame.title {
                node = clone_with_title(&node, title);
            }
            set_position(&mut node, None);
            if let Some(parent) = stack.last_mut() {
                parent.output.push(node);
            } else {
                root_output.push(node);
            }
            continue;
        }

        if root_index < root.children.len() {
            stack.push(create_frame(&root.children[root_index]));
            root_index += 1;
            continue;
        }
        return Root {
            node_type: root.node_type.clone(),
            position: None,
            children: root_output,
        };
    }
}
