use yozora_ast::{Node, Text};

pub(crate) fn parse_text_node(value: &str, position: Option<yozora_ast::Position>) -> Node {
    Node::Text(Text {
        position,
        value: value.to_string(),
    })
}
