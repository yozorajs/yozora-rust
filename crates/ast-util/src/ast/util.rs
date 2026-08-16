use serde_json::Value;
use yozora_ast::{Node, Position};

pub(crate) fn clone_with_children(node: &Node, children: Vec<Node>) -> Node {
    match node {
        Node::Admonition(value) => Node::Admonition(yozora_ast::Admonition {
            position: value.position.clone(),
            keyword: value.keyword.clone(),
            title: value.title.clone(),
            children,
        }),
        Node::Blockquote(value) => Node::Blockquote(yozora_ast::Blockquote {
            position: value.position.clone(),
            children,
        }),
        Node::Delete(value) => Node::Delete(yozora_ast::DeleteNode {
            position: value.position.clone(),
            children,
        }),
        Node::Emphasis(value) => Node::Emphasis(yozora_ast::Emphasis {
            position: value.position.clone(),
            children,
        }),
        Node::Footnote(value) => Node::Footnote(yozora_ast::Footnote {
            position: value.position.clone(),
            children,
        }),
        Node::FootnoteDefinition(value) => {
            Node::FootnoteDefinition(yozora_ast::FootnoteDefinition {
                position: value.position.clone(),
                identifier: value.identifier.clone(),
                label: value.label.clone(),
                children,
            })
        }
        Node::Heading(value) => Node::Heading(yozora_ast::Heading {
            position: value.position.clone(),
            identifier: value.identifier.clone(),
            depth: value.depth,
            children,
        }),
        Node::Link(value) => Node::Link(yozora_ast::Link {
            position: value.position.clone(),
            url: value.url.clone(),
            title: value.title.clone(),
            children,
        }),
        Node::LinkReference(value) => Node::LinkReference(yozora_ast::LinkReference {
            position: value.position.clone(),
            identifier: value.identifier.clone(),
            label: value.label.clone(),
            reference_type: value.reference_type,
            children,
        }),
        Node::List(value) => Node::List(yozora_ast::List {
            position: value.position.clone(),
            ordered: value.ordered,
            order_type: value.order_type.clone(),
            start: value.start,
            marker: value.marker,
            spread: value.spread,
            children,
        }),
        Node::ListItem(value) => Node::ListItem(yozora_ast::ListItem {
            position: value.position.clone(),
            status: value.status,
            children,
        }),
        Node::Paragraph(value) => Node::Paragraph(yozora_ast::Paragraph {
            position: value.position.clone(),
            children,
        }),
        Node::Strong(value) => Node::Strong(yozora_ast::Strong {
            position: value.position.clone(),
            children,
        }),
        Node::Table(value) => Node::Table(yozora_ast::Table {
            position: value.position.clone(),
            columns: value.columns.clone(),
            children,
        }),
        Node::TableRow(value) => Node::TableRow(yozora_ast::TableRow {
            position: value.position.clone(),
            children,
        }),
        Node::TableCell(value) => Node::TableCell(yozora_ast::TableCell {
            position: value.position.clone(),
            children,
        }),
        Node::Custom(value) => Node::Custom(yozora_ast::CustomNode {
            children: Some(children),
            ..value.clone()
        }),
        _ => node.clone(),
    }
}

pub(crate) fn clone_with_title(node: &Node, title: Vec<Node>) -> Node {
    match node {
        Node::Admonition(value) => Node::Admonition(yozora_ast::Admonition {
            position: value.position.clone(),
            keyword: value.keyword.clone(),
            title,
            children: value.children.clone(),
        }),
        _ => node.clone(),
    }
}

pub(crate) fn set_position(node: &mut Node, position: Option<Position>) {
    match node {
        Node::Admonition(node) => node.position = position,
        Node::Blockquote(node) => node.position = position,
        Node::Break(node) => node.position = position,
        Node::Code(node) => node.position = position,
        Node::Definition(node) => node.position = position,
        Node::Delete(node) => node.position = position,
        Node::EcmaImport(node) => node.position = position,
        Node::Emphasis(node) => node.position = position,
        Node::Footnote(node) => node.position = position,
        Node::FootnoteDefinition(node) => node.position = position,
        Node::FootnoteReference(node) => node.position = position,
        Node::Frontmatter(node) => node.position = position,
        Node::Heading(node) => node.position = position,
        Node::Html(node) => node.position = position,
        Node::Image(node) => node.position = position,
        Node::ImageReference(node) => node.position = position,
        Node::InlineCode(node) => node.position = position,
        Node::InlineMath(node) => node.position = position,
        Node::Link(node) => node.position = position,
        Node::LinkReference(node) => node.position = position,
        Node::List(node) => node.position = position,
        Node::ListItem(node) => node.position = position,
        Node::Math(node) => node.position = position,
        Node::Paragraph(node) => node.position = position,
        Node::Strong(node) => node.position = position,
        Node::Table(node) => node.position = position,
        Node::TableRow(node) => node.position = position,
        Node::TableCell(node) => node.position = position,
        Node::Text(node) => node.position = position,
        Node::ThematicBreak(node) => node.position = position,
        Node::Custom(node) => node.position = position,
    }
}

pub(crate) fn literal_value(node: &Node) -> Option<&str> {
    match node {
        Node::Code(node) => Some(&node.value),
        Node::Frontmatter(node) => Some(&node.value),
        Node::Html(node) => Some(&node.value),
        Node::InlineCode(node) => Some(&node.value),
        Node::InlineMath(node) => Some(&node.value),
        Node::Math(node) => Some(&node.value),
        Node::Text(node) => Some(&node.value),
        Node::Custom(node) => node.data.get("value").and_then(Value::as_str),
        _ => None,
    }
}

pub(crate) fn clone_with_literal_value(node: &Node, value: String) -> Node {
    match node {
        Node::Code(node) => Node::Code(yozora_ast::Code {
            value,
            ..node.clone()
        }),
        Node::Frontmatter(node) => Node::Frontmatter(yozora_ast::Frontmatter {
            value,
            ..node.clone()
        }),
        Node::Html(node) => Node::Html(yozora_ast::Html {
            value,
            ..node.clone()
        }),
        Node::InlineCode(node) => Node::InlineCode(yozora_ast::InlineCode {
            value,
            ..node.clone()
        }),
        Node::InlineMath(node) => Node::InlineMath(yozora_ast::InlineMath {
            value,
            ..node.clone()
        }),
        Node::Math(node) => Node::Math(yozora_ast::Math {
            value,
            ..node.clone()
        }),
        Node::Text(node) => Node::Text(yozora_ast::Text {
            value,
            ..node.clone()
        }),
        Node::Custom(node) => {
            let mut node = node.clone();
            node.data.insert("value".to_string(), Value::String(value));
            Node::Custom(node)
        }
        _ => node.clone(),
    }
}
