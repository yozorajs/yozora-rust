use yozora_ast::{List, ListItem, Node, Paragraph, Point, Position};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

use crate::r#match::TokenData;

pub(crate) fn parse_list_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());
    let mut list_item_tokens: Vec<&BlockToken> = Vec::new();

    for token in tokens {
        let Some(data) = token.data_as::<TokenData>() else {
            continue;
        };

        if list_item_tokens.is_empty() {
            list_item_tokens.push(token);
            continue;
        }

        let Some(first_data) = list_item_tokens[0].data_as::<TokenData>() else {
            list_item_tokens.clear();
            list_item_tokens.push(token);
            continue;
        };

        if first_data.ordered == data.ordered
            && first_data.order_type == data.order_type
            && first_data.marker == data.marker
        {
            list_item_tokens.push(token);
        } else {
            if let Some(node) = resolve_list(&list_item_tokens, parse_api) {
                nodes.push(node);
            }

            list_item_tokens.clear();
            list_item_tokens.push(token);
        }
    }

    if let Some(node) = resolve_list(&list_item_tokens, parse_api) {
        nodes.push(node);
    }

    nodes
}

fn resolve_list(tokens: &[&BlockToken], parse_api: &dyn ParseBlockPhaseApi) -> Option<Node> {
    let first_token = *tokens.first()?;
    let first_data = first_token.data_as::<TokenData>()?;
    let spread = calc_spread(tokens);

    let mut children = Vec::with_capacity(tokens.len());
    for token in tokens {
        let data = token.data_as::<TokenData>()?;
        let item_nodes = parse_api.parseBlockTokens(Some(&token.children));
        let item_children = if spread {
            item_nodes
        } else {
            flatten_paragraph_nodes(item_nodes)
        };

        children.push(Node::ListItem(ListItem {
            position: if parse_api.shouldReservePosition() {
                token.position.clone()
            } else {
                None
            },
            status: data.status,
            children: item_children,
        }));
    }

    let position = if parse_api.shouldReservePosition() {
        calc_list_position(tokens)
    } else {
        None
    };

    Some(Node::List(List {
        position,
        ordered: first_data.ordered,
        order_type: first_data.order_type.clone(),
        start: first_data.order,
        marker: first_data.marker,
        spread,
        children,
    }))
}

fn calc_spread(tokens: &[&BlockToken]) -> bool {
    for token in tokens {
        if token.children.len() > 1 && has_spread_between_children(&token.children) {
            return true;
        }
    }

    if tokens.len() > 1 {
        let mut previous = tokens[0].position.as_ref();
        for token in &tokens[1..] {
            let current = token.position.as_ref();
            if let (Some(prev), Some(curr)) = (previous, current) {
                if prev.end.line + 1 < curr.start.line {
                    return true;
                }
            }

            if current.is_some() {
                previous = current;
            }
        }
    }

    false
}

fn has_spread_between_children(children: &[BlockToken]) -> bool {
    if children.len() <= 1 {
        return false;
    }

    let mut previous = children[0].position.as_ref();
    for child in &children[1..] {
        let current = child.position.as_ref();
        if let (Some(prev), Some(curr)) = (previous, current) {
            if prev.end.line + 1 < curr.start.line {
                return true;
            }
        }

        if current.is_some() {
            previous = current;
        }
    }

    false
}

fn flatten_paragraph_nodes(nodes: Vec<Node>) -> Vec<Node> {
    let mut flattened = Vec::new();
    for node in nodes {
        match node {
            Node::Paragraph(Paragraph { children, .. }) => flattened.extend(children),
            _ => flattened.push(node),
        }
    }
    flattened
}

fn calc_list_position(tokens: &[&BlockToken]) -> Option<Position> {
    let first = tokens.first()?.position.as_ref()?;
    let last = tokens.last()?.position.as_ref()?;

    Some(Position {
        start: Point {
            line: first.start.line,
            column: first.start.column,
            offset: first.start.offset,
        },
        end: Point {
            line: last.end.line,
            column: last.end.column,
            offset: last.end.offset,
        },
        indent: None,
    })
}
