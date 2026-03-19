use yozora_ast::{Link, Node, Text};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub(crate) struct LinkTokenData {
    pub url: String,
    pub title: Option<String>,
    pub label: String,
    pub children_tokens: Vec<InlineToken>,
}

pub(crate) fn parse_link_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<LinkTokenData>() else {
            continue;
        };

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
            url: parse_api.formatUrl(&data.url),
            title: data.title.clone(),
            children: if data.children_tokens.is_empty() {
                vec![Node::Text(Text {
                    position: None,
                    value: data.label.clone(),
                })]
            } else {
                parse_api.parseInlineTokens(Some(&data.children_tokens))
            },
        }));
    }

    nodes
}
