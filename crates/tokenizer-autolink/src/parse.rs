use yozora_ast::{Link, Node};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutolinkContentType {
    Uri,
    Email,
}

#[derive(Debug, Clone)]
pub(crate) struct AutolinkTokenData {
    pub content_type: AutolinkContentType,
    pub children_tokens: Vec<InlineToken>,
}

pub(crate) fn parse_autolink_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let node_points = parse_api.getNodePoints();
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

        let position = if parse_api.shouldReservePosition() {
            Some(parse_api.calcPosition(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            }))
        } else {
            None
        };

        nodes.push(Node::Link(Link {
            position,
            url: parse_api.formatUrl(&url),
            title: None,
            children: parse_api.parseInlineTokens(Some(&data.children_tokens)),
        }));
    }

    nodes
}
