use yozora_ast::{Node, Text};

pub(crate) fn parse_backslash_escaped_text(decoded: String) -> Vec<Node> {
    vec![Node::Text(Text {
        position: None,
        value: decoded,
    })]
}
