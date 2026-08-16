use yozora_ast::{Link, Node, Text};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

use crate::types::{AutolinkContentType, AutolinkTokenData};

pub(crate) fn parse_autolink_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let node_points = parse_api.get_node_points();
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<AutolinkTokenData>() else {
            continue;
        };
        if token.end_index <= token.start_index + 1 || token.end_index > node_points.len() {
            continue;
        }

        let mut url = calc_string_from_node_points(
            node_points,
            token.start_index + 1,
            token.end_index - 1,
            false,
        );
        if data.content_type == AutolinkContentType::Email {
            url = format!("mailto:{url}");
        }

        let (position, text_position) = if parse_api.should_reserve_position() {
            (
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                })),
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index + 1,
                    end_index: token.end_index - 1,
                })),
            )
        } else {
            (None, None)
        };

        nodes.push(Node::Link(Link {
            position,
            url: parse_api.format_url(&url),
            title: None,
            children: vec![Node::Text(Text {
                position: text_position,
                value: calc_string_from_node_points(
                    node_points,
                    token.start_index + 1,
                    token.end_index - 1,
                    false,
                ),
            })],
        }));
    }

    nodes
}
