use yozora_ast::Node;

pub fn calc_image_alt(nodes: &[Node]) -> String {
    nodes.iter().map(calc_node_alt).collect()
}

fn calc_node_alt(node: &Node) -> String {
    match node {
        Node::Code(node) => node.value.clone(),
        Node::Html(node) => node.value.clone(),
        Node::InlineCode(node) => node.value.clone(),
        Node::InlineMath(node) => node.value.clone(),
        Node::Math(node) => node.value.clone(),
        Node::Text(node) => node.value.clone(),

        Node::Image(node) => node.alt.clone(),
        Node::ImageReference(node) => node.alt.clone(),

        Node::Admonition(node) => calc_image_alt(&node.children),
        Node::Blockquote(node) => calc_image_alt(&node.children),
        Node::Delete(node) => calc_image_alt(&node.children),
        Node::Emphasis(node) => calc_image_alt(&node.children),
        Node::Footnote(node) => calc_image_alt(&node.children),
        Node::FootnoteDefinition(node) => calc_image_alt(&node.children),
        Node::Heading(node) => calc_image_alt(&node.children),
        Node::Link(node) => calc_image_alt(&node.children),
        Node::LinkReference(node) => calc_image_alt(&node.children),
        Node::List(node) => calc_image_alt(&node.children),
        Node::ListItem(node) => calc_image_alt(&node.children),
        Node::Paragraph(node) => calc_image_alt(&node.children),
        Node::Strong(node) => calc_image_alt(&node.children),
        Node::Table(node) => calc_image_alt(&node.children),
        Node::TableCell(node) => calc_image_alt(&node.children),
        Node::TableRow(node) => calc_image_alt(&node.children),

        Node::Break(_)
        | Node::Definition(_)
        | Node::EcmaImport(_)
        | Node::FootnoteReference(_)
        | Node::Frontmatter(_)
        | Node::ThematicBreak(_)
        | Node::Custom(_) => String::new(),
    }
}
