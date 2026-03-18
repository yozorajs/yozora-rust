use yozora_ast::{BreakNode, Node, Text};
use yozora_core_tokenizer::{NodeInterval, ParseInlinePhaseApi};

use crate::r#match::BreakToken;

pub(crate) fn parse_break_tokens(
    input: &str,
    tokens: &[BreakToken],
    parse_api: Option<&dyn ParseInlinePhaseApi>,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            BreakToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: calc_position(parse_api, *interval),
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            BreakToken::Break(interval) => {
                nodes.push(Node::Break(BreakNode {
                    position: calc_position(parse_api, *interval),
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
