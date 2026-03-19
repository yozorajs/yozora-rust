use yozora_ast::{Image, Node};
use yozora_character::{calc_escaped_string_from_node_points, AsciiCodePoint};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

use crate::util::calc_image_alt;

#[derive(Debug, Clone)]
pub(crate) struct ImageTokenData {
    pub destination_content: Option<NodeInterval>,
    pub title_content: Option<NodeInterval>,
    pub children_tokens: Vec<InlineToken>,
}

pub(crate) fn parse_image_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());
    let node_points = parse_api.get_node_points();

    for token in tokens {
        let Some(data) = token.data_as::<ImageTokenData>() else {
            continue;
        };

        let mut url = String::new();
        if let Some(destination_content) = data.destination_content {
            let mut start_index = destination_content.start_index;
            let mut end_index = destination_content.end_index;

            if start_index < end_index
                && node_points[start_index].code_point == AsciiCodePoint::OPEN_ANGLE as i32
            {
                start_index += 1;
                end_index = end_index.saturating_sub(1);
            }

            let destination =
                calc_escaped_string_from_node_points(node_points, start_index, end_index, true);
            url = parse_api.format_url(&destination);
        }

        let children = parse_api.parse_inline_tokens(&data.children_tokens);
        let alt = calc_image_alt(&children);

        let title = data.title_content.map(|title_content| {
            let start_index = title_content.start_index.saturating_add(1);
            let end_index = title_content.end_index.saturating_sub(1);
            if start_index >= end_index {
                String::new()
            } else {
                calc_escaped_string_from_node_points(node_points, start_index, end_index, false)
            }
        });

        let position = if parse_api.should_reserve_position() {
            parse_api.calc_position(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            })
        } else {
            None
        };

        nodes.push(Node::Image(Image {
            position,
            url,
            title,
            alt,
        }));
    }

    nodes
}
