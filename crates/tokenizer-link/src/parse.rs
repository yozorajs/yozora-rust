use yozora_ast::{Link, Node, Text};

use crate::r#match::LinkToken;

pub(crate) fn parse_link_tokens(input: &str, tokens: &[LinkToken]) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            LinkToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            LinkToken::Link {
                url, title, label, ..
            } => {
                nodes.push(Node::Link(Link {
                    position: None,
                    url: url.clone(),
                    title: title.clone(),
                    children: vec![Node::Text(Text {
                        position: None,
                        value: label.clone(),
                    })],
                }));
            }
        }
    }

    nodes
}
