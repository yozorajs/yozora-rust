use yozora_ast::{Node, Root};

pub fn collect_nodes(root: &Node) -> Vec<&Node> {
    let mut out = Vec::new();
    collect_nodes_impl(root, &mut out);
    out
}

pub fn collect_root_nodes(root: &Root) -> Vec<&Node> {
    let mut out = Vec::new();
    for child in &root.children {
        collect_nodes_impl(child, &mut out);
    }
    out
}

fn collect_nodes_impl<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    out.push(node);
    match node {
        Node::Admonition(n) => {
            for child in &n.title {
                collect_nodes_impl(child, out);
            }
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Blockquote(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Delete(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Emphasis(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Footnote(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::FootnoteDefinition(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Heading(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Link(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::LinkReference(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::ListItem(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Paragraph(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Strong(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::TableCell(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::List(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::Table(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        Node::TableRow(n) => {
            for child in &n.children {
                collect_nodes_impl(child, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::{Paragraph, Text};

    #[test]
    fn collect_nodes_should_include_self_and_children() {
        let root = Node::Paragraph(Paragraph {
            position: None,
            children: vec![Node::Text(Text {
                position: None,
                value: "hello".to_string(),
            })],
        });

        let nodes = collect_nodes(&root);
        assert_eq!(nodes.len(), 2);
    }
}
