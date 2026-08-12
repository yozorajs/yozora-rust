use yozora_ast::{Link, Node, Text};
use yozora_character::{calc_escaped_string_from_node_points, AsciiCodePoint};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub struct LinkTokenData {
    pub destination_content: Option<NodeInterval>,
    pub title_content: Option<NodeInterval>,
    pub(crate) label: String,
}

pub(crate) fn parse_link_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());
    let node_points = parse_api.get_node_points();

    for token in tokens {
        let Some(data) = token.data_as::<LinkTokenData>() else {
            continue;
        };

        let position = if parse_api.should_reserve_position() {
            Some(parse_api.calc_position(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
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

        let title = data.title_content.and_then(|title_content| {
            (title_content.end_index >= title_content.start_index + 2).then(|| {
                calc_escaped_string_from_node_points(
                    node_points,
                    title_content.start_index + 1,
                    title_content.end_index - 1,
                    false,
                )
            })
        });

        nodes.push(Node::Link(Link {
            position,
            url,
            title,
            children: if token.children.is_empty() {
                vec![Node::Text(Text {
                    position: None,
                    value: data.label.clone(),
                })]
            } else {
                parse_api.parse_inline_tokens(Some(&token.children))
            },
        }));
    }

    nodes
}
