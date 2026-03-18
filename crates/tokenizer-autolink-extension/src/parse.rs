use yozora_ast::{Link, Node, Text};
use yozora_core_tokenizer::ParseInlinePhaseApi;

use crate::r#match::AutolinkExtensionToken;

pub(crate) fn parse_autolink_extension_tokens(
    input: &str,
    tokens: &[AutolinkExtensionToken],
    parse_api: Option<&dyn ParseInlinePhaseApi>,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            AutolinkExtensionToken::Text(interval) => {
                let position = parse_api.and_then(|api| api.calc_position(*interval));
                nodes.push(Node::Text(Text {
                    position,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            AutolinkExtensionToken::Link {
                interval,
                display,
                href,
            } => {
                let position = parse_api.and_then(|api| api.calc_position(*interval));
                nodes.push(Node::Link(Link {
                    position: position.clone(),
                    url: href.clone(),
                    title: None,
                    children: vec![Node::Text(Text {
                        position,
                        value: display.clone(),
                    })],
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
