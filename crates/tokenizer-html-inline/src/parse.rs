use yozora_ast::{Html, Node};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

pub(crate) fn parse_html_inline_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let node_points = parse_api.getNodePoints();
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        if token.start_index >= token.end_index || token.end_index > node_points.len() {
            continue;
        }

        let value =
            calc_string_from_node_points(node_points, token.start_index, token.end_index, false);

        let position = if parse_api.shouldReservePosition() {
            Some(parse_api.calcPosition(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
        };

        nodes.push(Node::Html(Html { position, value }));
    }

    nodes
}
