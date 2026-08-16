use yozora_ast::Node;

pub fn calc_image_alt(nodes: &[Node]) -> String {
    let mut result = String::new();
    let mut stack: Vec<&Node> = nodes.iter().rev().collect();

    while let Some(node) = stack.pop() {
        match node {
            Node::Code(node) => result.push_str(&node.value),
            Node::Html(node) => result.push_str(&node.value),
            Node::InlineCode(node) => result.push_str(&node.value),
            Node::InlineMath(node) => result.push_str(&node.value),
            Node::Math(node) => result.push_str(&node.value),
            Node::Text(node) => result.push_str(&node.value),

            Node::Image(node) => result.push_str(&node.alt),
            Node::ImageReference(node) => result.push_str(&node.alt),

            Node::Admonition(node) => stack.extend(node.children.iter().rev()),
            Node::Blockquote(node) => stack.extend(node.children.iter().rev()),
            Node::Delete(node) => stack.extend(node.children.iter().rev()),
            Node::Emphasis(node) => stack.extend(node.children.iter().rev()),
            Node::Footnote(node) => stack.extend(node.children.iter().rev()),
            Node::FootnoteDefinition(node) => stack.extend(node.children.iter().rev()),
            Node::Heading(node) => stack.extend(node.children.iter().rev()),
            Node::Link(node) => stack.extend(node.children.iter().rev()),
            Node::LinkReference(node) => stack.extend(node.children.iter().rev()),
            Node::List(node) => stack.extend(node.children.iter().rev()),
            Node::ListItem(node) => stack.extend(node.children.iter().rev()),
            Node::Paragraph(node) => stack.extend(node.children.iter().rev()),
            Node::Strong(node) => stack.extend(node.children.iter().rev()),
            Node::Table(node) => stack.extend(node.children.iter().rev()),
            Node::TableCell(node) => stack.extend(node.children.iter().rev()),
            Node::TableRow(node) => stack.extend(node.children.iter().rev()),

            Node::Break(_)
            | Node::Definition(_)
            | Node::EcmaImport(_)
            | Node::FootnoteReference(_)
            | Node::Frontmatter(_)
            | Node::ThematicBreak(_)
            | Node::Custom(_) => {}
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::{Emphasis, Image, Text};

    #[test]
    fn collects_literal_values_and_nested_image_alt_in_source_order() {
        let text = Node::Text(Text {
            position: None,
            value: "foo".to_string(),
        });
        let nested_text = Node::Text(Text {
            position: None,
            value: "bar".to_string(),
        });
        let image = Node::Image(Image {
            position: None,
            url: "/image".to_string(),
            title: None,
            alt: "baz".to_string(),
        });
        let emphasis = Node::Emphasis(Emphasis {
            position: None,
            children: vec![nested_text, image],
        });

        assert_eq!(calc_image_alt(&[text, emphasis]), "foobarbaz");
    }

    #[test]
    fn handles_deep_nodes_without_recursive_stack_growth() {
        let mut node = Node::Text(Text {
            position: None,
            value: "value".to_string(),
        });
        for _ in 0..10_000 {
            node = Node::Emphasis(Emphasis {
                position: None,
                children: vec![node],
            });
        }

        assert_eq!(calc_image_alt(std::slice::from_ref(&node)), "value");
        std::mem::forget(node);
    }
}
