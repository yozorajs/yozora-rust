use yozora_ast::{DeleteNode, Node, Text};

use crate::r#match::DeleteToken;

pub(crate) fn parse_delete_tokens(input: &str, tokens: &[DeleteToken]) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            DeleteToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            DeleteToken::Delete { content, .. } => {
                nodes.push(Node::Delete(DeleteNode {
                    position: None,
                    children: vec![Node::Text(Text {
                        position: None,
                        value: content.clone(),
                    })],
                }));
            }
        }
    }

    nodes
}
