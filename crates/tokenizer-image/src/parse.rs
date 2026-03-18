use yozora_ast::{Image, Node, Text};

use crate::r#match::ImageToken;

pub(crate) fn parse_image_tokens(input: &str, tokens: &[ImageToken]) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            ImageToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            ImageToken::Image {
                url, title, alt, ..
            } => {
                nodes.push(Node::Image(Image {
                    position: None,
                    url: url.clone(),
                    title: title.clone(),
                    alt: alt.clone(),
                }));
            }
        }
    }

    nodes
}
