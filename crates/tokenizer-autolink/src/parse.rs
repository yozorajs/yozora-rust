use yozora_ast::{Link, Node, Text};

use crate::r#match::AutolinkToken;

pub(crate) fn parse_autolink_tokens(input: &str, tokens: &[AutolinkToken]) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            AutolinkToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            AutolinkToken::Link { url, label, .. } => {
                nodes.push(Node::Link(Link {
                    position: None,
                    url: url.clone(),
                    title: None,
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
