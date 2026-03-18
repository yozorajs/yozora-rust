use yozora_ast::{Footnote, Node, Text};

use crate::r#match::FootnoteToken;

pub(crate) fn parse_footnote_tokens(input: &str, tokens: &[FootnoteToken]) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            FootnoteToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            FootnoteToken::Literal(value) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: value.clone(),
                }));
            }
            FootnoteToken::Footnote { content, .. } => {
                nodes.push(Node::Footnote(Footnote {
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
