use yozora_ast::{List, ListItem, Node, Paragraph, Point, Position, TaskStatus};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi, ParseBlockTask, ParseBlockTaskStep};

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
        let item_nodes = parse_api.parse_block_tokens(Some(&token.children));
        let item_children = if spread {
            item_nodes
        } else {
            flatten_paragraph_nodes(item_nodes)
        };

        children.push(Node::ListItem(ListItem {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            status: data.status,
            children: item_children,
        }));
    }

    let position = if parse_api.should_reserve_position() {
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
                if prev.end.line + usize::from(prev.end.column != 1) < curr.start.line {
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
            if prev.end.line + usize::from(prev.end.column != 1) < curr.start.line {
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

struct PendingListItem {
    position: Option<Position>,
    status: Option<TaskStatus>,
    children: Vec<BlockToken>,
}

struct PendingList {
    position: Option<Position>,
    ordered: bool,
    order_type: Option<String>,
    start: Option<usize>,
    marker: u32,
    spread: bool,
    items: Vec<PendingListItem>,
}

struct ListParseTask {
    pending: Vec<PendingList>,
    list_index: usize,
    item_index: usize,
    waiting_for_children: bool,
    item_nodes: Vec<Node>,
    nodes: Vec<Node>,
}

impl ParseBlockTask for ListParseTask {
    fn resume(&mut self, children: Option<Vec<Node>>) -> ParseBlockTaskStep {
        if self.waiting_for_children {
            let list = &self.pending[self.list_index];
            let item = &list.items[self.item_index - 1];
            let children = children.expect("list task should resume with children");
            self.item_nodes.push(Node::ListItem(ListItem {
                position: item.position.clone(),
                status: item.status,
                children: if list.spread {
                    children
                } else {
                    flatten_paragraph_nodes(children)
                },
            }));
            self.waiting_for_children = false;
        }

        loop {
            if self.list_index >= self.pending.len() {
                return ParseBlockTaskStep::Done(std::mem::take(&mut self.nodes));
            }

            let list = &self.pending[self.list_index];
            if self.item_index >= list.items.len() {
                self.nodes.push(Node::List(List {
                    position: list.position.clone(),
                    ordered: list.ordered,
                    order_type: list.order_type.clone(),
                    start: list.start,
                    marker: list.marker,
                    spread: list.spread,
                    children: std::mem::take(&mut self.item_nodes),
                }));
                self.list_index += 1;
                self.item_index = 0;
                continue;
            }

            let children = list.items[self.item_index].children.clone();
            self.item_index += 1;
            self.waiting_for_children = true;
            return ParseBlockTaskStep::Request(children);
        }
    }
}

pub(crate) fn create_list_parse_task(
    tokens: &[BlockToken],
    should_reserve_position: bool,
) -> Box<dyn ParseBlockTask> {
    let mut pending = Vec::new();
    let mut list_item_tokens: Vec<&BlockToken> = Vec::new();
    for token in tokens {
        let Some(data) = token.data_as::<TokenData>() else {
            continue;
        };
        let is_same_list = list_item_tokens
            .first()
            .and_then(|first| first.data_as::<TokenData>())
            .is_some_and(|first| {
                first.ordered == data.ordered
                    && first.order_type == data.order_type
                    && first.marker == data.marker
            });
        if !list_item_tokens.is_empty() && !is_same_list {
            if let Some(list) = build_pending_list(&list_item_tokens, should_reserve_position) {
                pending.push(list);
            }
            list_item_tokens.clear();
        }
        list_item_tokens.push(token);
    }
    if let Some(list) = build_pending_list(&list_item_tokens, should_reserve_position) {
        pending.push(list);
    }

    let capacity = pending.len();
    Box::new(ListParseTask {
        pending,
        list_index: 0,
        item_index: 0,
        waiting_for_children: false,
        item_nodes: Vec::new(),
        nodes: Vec::with_capacity(capacity),
    })
}

fn build_pending_list(
    tokens: &[&BlockToken],
    should_reserve_position: bool,
) -> Option<PendingList> {
    let first_token = *tokens.first()?;
    let first_data = first_token.data_as::<TokenData>()?;
    let spread = calc_spread(tokens);
    let items = tokens
        .iter()
        .filter_map(|token| {
            let data = token.data_as::<TokenData>()?;
            Some(PendingListItem {
                position: if should_reserve_position {
                    token.position.clone()
                } else {
                    None
                },
                status: data.status,
                children: token.children.to_vec(),
            })
        })
        .collect();
    Some(PendingList {
        position: if should_reserve_position {
            calc_list_position(tokens)
        } else {
            None
        },
        ordered: first_data.ordered,
        order_type: first_data.order_type.clone(),
        start: first_data.order,
        marker: first_data.marker,
        spread,
        items,
    })
}
