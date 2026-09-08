use yozora_ast::{Link, Node};
use yozora_character::{calc_escaped_string_from_node_points, AsciiCodePoint};
use yozora_core_tokenizer::{
    parse_inline_containers, InlineToken, NodeInterval, ParseInlineHookResult, ParseInlinePhaseApi,
};

use crate::types::LinkTokenData;

pub(crate) fn parse_link_tokens<'a>(
    tokens: &'a [InlineToken],
    parse_api: &'a dyn ParseInlinePhaseApi,
) -> ParseInlineHookResult<'a> {
    let node_points = parse_api.get_node_points();
    parse_inline_containers(
        tokens,
        move |token| {
            let data = token.data_as::<LinkTokenData>()?;

            let position = if parse_api.should_reserve_position() {
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            let destination = if let Some(destination_content) = data.destination_content {
                let mut start_index = destination_content.start_index;
                let mut end_index = destination_content.end_index;
                if start_index < end_index
                    && node_points[start_index].code_point == AsciiCodePoint::OPEN_ANGLE as i32
                {
                    start_index += 1;
                    end_index = end_index.saturating_sub(1);
                }
                calc_escaped_string_from_node_points(node_points, start_index, end_index, true)
            } else {
                String::new()
            };
            let url = parse_api.format_url(&destination);

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

            Some(Link {
                position,
                url,
                title,
                children: Vec::new(),
            })
        },
        |mut node, children| {
            node.children = children;
            Node::Link(node)
        },
    )
}
