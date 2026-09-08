use yozora_ast::{Link, Node};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{
    parse_inline_containers, InlineToken, NodeInterval, ParseInlineHookResult, ParseInlinePhaseApi,
};

use crate::types::{AutolinkExtensionContentType, AutolinkExtensionTokenData};

pub(crate) fn parse_autolink_extension_tokens<'a>(
    tokens: &'a [InlineToken],
    parse_api: &'a dyn ParseInlinePhaseApi,
) -> ParseInlineHookResult<'a> {
    let node_points = parse_api.get_node_points();
    parse_inline_containers(
        tokens,
        move |token| {
            let data = token.data_as::<AutolinkExtensionTokenData>()?;

            if token.start_index >= token.end_index || token.end_index > node_points.len() {
                return None;
            }

            let mut url = calc_string_from_node_points(
                node_points,
                token.start_index,
                token.end_index,
                false,
            );
            match data.content_type {
                AutolinkExtensionContentType::Email => {
                    url = format!("mailto:{url}");
                }
                AutolinkExtensionContentType::UriWww => {
                    url = format!("http://{url}");
                }
                AutolinkExtensionContentType::Uri => {}
            }

            let position = if parse_api.should_reserve_position() {
                Some(parse_api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            Some(Link {
                position,
                url: parse_api.format_url(&url),
                title: None,
                children: Vec::new(),
            })
        },
        |mut node, children| {
            node.children = children;
            Node::Link(node)
        },
    )
}
