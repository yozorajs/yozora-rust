use yozora_ast::{Node, Root};

pub fn mutate_node<F>(node: &mut Node, visitor: &mut F)
where
    F: FnMut(&mut Node),
{
    visitor(node);
    match node {
        Node::Admonition(n) => {
            for child in &mut n.title {
                mutate_node(child, visitor);
            }
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Blockquote(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Delete(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Emphasis(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Footnote(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::FootnoteDefinition(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Heading(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Link(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::LinkReference(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::List(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::ListItem(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Paragraph(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Strong(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::Table(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::TableRow(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        Node::TableCell(n) => {
            for child in &mut n.children {
                mutate_node(child, visitor);
            }
        }
        _ => {}
    }
}

pub fn mutate_root<F>(root: &mut Root, visitor: &mut F)
where
    F: FnMut(&mut Node),
{
    for child in &mut root.children {
        mutate_node(child, visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::{Paragraph, Text};

    #[test]
    fn mutate_node_should_visit_descendants() {
        let mut root = Node::Paragraph(Paragraph {
            position: None,
            children: vec![Node::Text(Text {
                position: None,
                value: "hello".to_string(),
            })],
        });

        let mut count = 0usize;
        mutate_node(&mut root, &mut |node| {
            count += 1;
            if let Node::Text(text) = node {
                text.value = text.value.to_uppercase();
            }
        });

        assert_eq!(count, 2);
        let Node::Paragraph(paragraph) = root else {
            panic!("expected paragraph root");
        };
        let Node::Text(text) = &paragraph.children[0] else {
            panic!("expected text child");
        };
        assert_eq!(text.value, "HELLO");
    }
}
