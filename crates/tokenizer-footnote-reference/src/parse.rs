use yozora_ast::{FootnoteReference, Node, Text};
use yozora_core_tokenizer::{NodeInterval, ParseInlinePhaseApi};

use crate::r#match::FootnoteReferenceToken;

pub(crate) fn parse_footnote_reference_tokens(
    input: &str,
    tokens: &[FootnoteReferenceToken],
    parse_api: Option<&dyn ParseInlinePhaseApi>,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            FootnoteReferenceToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: calc_position(parse_api, *interval),
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            FootnoteReferenceToken::Reference {
                interval,
                identifier,
                label,
            } => {
                nodes.push(Node::FootnoteReference(FootnoteReference {
                    position: calc_position(parse_api, *interval),
                    identifier: identifier.clone(),
                    label: label.clone(),
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
