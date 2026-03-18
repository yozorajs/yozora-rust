use yozora_ast::{LinkReference, Node, Text};
use yozora_core_tokenizer::{NodeInterval, ParseInlinePhaseApi};

use crate::r#match::LinkReferenceToken;

pub(crate) fn parse_link_reference_tokens(
    input: &str,
    tokens: &[LinkReferenceToken],
    parse_api: Option<&dyn ParseInlinePhaseApi>,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            LinkReferenceToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: calc_position(parse_api, *interval),
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            LinkReferenceToken::Link {
                interval,
                child_interval,
                identifier,
                label,
                reference_type,
                child_text,
            } => {
                nodes.push(Node::LinkReference(LinkReference {
                    position: calc_position(parse_api, *interval),
                    identifier: identifier.clone(),
                    label: label.clone(),
                    reference_type: *reference_type,
                    children: vec![Node::Text(Text {
                        position: calc_position(parse_api, *child_interval),
                        value: child_text.clone(),
                    })],
                }));
            }
        }
    }

    nodes
}

fn calc_position(
    parse_api: Option<&dyn ParseInlinePhaseApi>,
    interval: NodeInterval,
) -> Option<yozora_ast::Position> {
    parse_api.and_then(|api| api.calc_position(interval))
}
