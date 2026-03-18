use yozora_ast::{Node, Paragraph};

pub(crate) fn parse_paragraph_node(
    inline_children: Vec<Node>,
    position: Option<yozora_ast::Position>,
) -> Node {
    Node::Paragraph(Paragraph {
        position,
        children: inline_children,
    })
}
