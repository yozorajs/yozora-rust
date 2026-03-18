use yozora_ast::{ImageReference, Node, Text};

use crate::r#match::ImageReferenceToken;

pub(crate) fn parse_image_reference_tokens(
    input: &str,
    tokens: &[ImageReferenceToken],
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            ImageReferenceToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            ImageReferenceToken::ImageReference {
                identifier,
                label,
                reference_type,
                alt,
                ..
            } => {
                nodes.push(Node::ImageReference(ImageReference {
                    position: None,
                    identifier: identifier.clone(),
                    label: label.clone(),
                    reference_type: reference_type.clone(),
                    alt: alt.clone(),
                }));
            }
        }
    }

    nodes
}
