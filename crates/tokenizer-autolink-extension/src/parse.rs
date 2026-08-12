use yozora_ast::{Link, Node};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutolinkExtensionContentType {
    Uri,
    UriWww,
    Email,
}

#[derive(Debug, Clone)]
pub struct AutolinkExtensionTokenData {
    pub content_type: AutolinkExtensionContentType,
}

pub(crate) fn parse_autolink_extension_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let node_points = parse_api.get_node_points();
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<AutolinkExtensionTokenData>() else {
            continue;
        };

        if token.start_index >= token.end_index || token.end_index > node_points.len() {
            continue;
        }

        let mut url =
            calc_string_from_node_points(node_points, token.start_index, token.end_index, false);
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

        nodes.push(Node::Link(Link {
            position,
            url: parse_api.format_url(&url),
            title: None,
            children: parse_api.parse_inline_tokens(Some(&token.children)),
        }));
    }

    nodes
}
