use yozora_ast::{Admonition, Node};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi, PhrasingContentLine};

#[derive(Debug, Clone)]
pub(crate) struct AdmonitionTokenData {
    pub keyword: String,
    pub marker_count: usize,
    pub indent: usize,
    pub title_line: Option<PhrasingContentLine>,
    pub body_lines: Vec<PhrasingContentLine>,
}

pub(crate) fn parse_admonition_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<AdmonitionTokenData>() else {
            continue;
        };

        let title = if let Some(title_line) = &data.title_line {
            if title_line.start_index < title_line.end_index {
                parse_api.processInlines(
                    &title_line.node_points[title_line.start_index..title_line.end_index],
                )
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let children = parse_api.parseBlockTokens(Some(&token.children));
        nodes.push(Node::Admonition(Admonition {
            position: if parse_api.shouldReservePosition() {
                token.position.clone()
            } else {
                None
            },
            keyword: data.keyword.clone(),
            title,
            children,
        }));
    }

    nodes
}
