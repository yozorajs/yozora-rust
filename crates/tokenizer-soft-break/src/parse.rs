use yozora_ast::{Node, Text};

pub(crate) fn parse_soft_break_text(value: String) -> Vec<Node> {
    vec![Node::Text(Text {
        position: None,
        value,
    })]
}
