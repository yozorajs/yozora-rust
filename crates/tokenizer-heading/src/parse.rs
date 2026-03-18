use yozora_ast::{Heading, Node, Text};

use crate::r#match::HeadingToken;

pub(crate) fn parse_heading_token(token: HeadingToken) -> Node {
    let children = if token.content.is_empty() {
        Vec::new()
    } else {
        vec![Node::Text(Text {
            position: None,
            value: token.content,
        })]
    };

    Node::Heading(Heading {
        position: None,
        identifier: None,
        depth: token.depth,
        children,
    })
}
