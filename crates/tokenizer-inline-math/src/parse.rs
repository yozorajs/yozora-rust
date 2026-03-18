use yozora_ast::{InlineMath, Node, Text};

use crate::r#match::InlineMathToken;

pub(crate) fn parse_inline_math_tokens(input: &str, tokens: &[InlineMathToken]) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            InlineMathToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            InlineMathToken::InlineMath { value, .. } => {
                nodes.push(Node::InlineMath(InlineMath {
                    position: None,
                    value: value.clone(),
                }));
            }
        }
    }

    merge_adjacent_text(nodes)
}

fn merge_adjacent_text(nodes: Vec<Node>) -> Vec<Node> {
    let mut merged = Vec::new();
    for node in nodes {
        match node {
            Node::Text(text) => {
                if let Some(Node::Text(last)) = merged.last_mut() {
                    last.value.push_str(&text.value);
                } else {
                    merged.push(Node::Text(text));
                }
            }
            other => merged.push(other),
        }
    }
    merged
}
