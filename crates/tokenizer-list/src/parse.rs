use yozora_ast::{List, ListItem, Node, NodeBuffer, Paragraph, Point, Position};
use yozora_core_tokenizer::{
    BlockToken, ParseBlockError, ParseBlockGenerator, ParseBlockGeneratorResult,
    ParseBlockGeneratorResume, ParseBlockHookResult, ParseBlockPhaseApi, ParseBlockResult,
};

use crate::types::ListTokenData;

pub(crate) fn parse_list_tokens<'a>(
    tokens: &'a [BlockToken],
    parse_api: &'a dyn ParseBlockPhaseApi,
) -> ParseBlockHookResult<'a> {
    ParseBlockHookResult::Generator(Box::new(ListParseGenerator {
        parse_api,
        groups: collect_list_item_groups(tokens),
        next_group_index: 0,
        current_list: None,
        nodes: NodeBuffer::with_capacity(tokens.len()),
        started: false,
    }))
}

fn collect_list_item_groups(tokens: &[BlockToken]) -> Vec<Vec<&BlockToken>> {
    let mut groups: Vec<Vec<&BlockToken>> = Vec::new();
    let mut list_item_tokens: Vec<&BlockToken> = Vec::new();

    for token in tokens {
        let Some(data) = token.data_as::<ListTokenData>() else {
            continue;
        };

        if list_item_tokens.is_empty() {
            list_item_tokens.push(token);
            continue;
        }

        let Some(first_data) = list_item_tokens[0].data_as::<ListTokenData>() else {
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
            groups.push(std::mem::take(&mut list_item_tokens));
            list_item_tokens.clear();
            list_item_tokens.push(token);
        }
    }

    if !list_item_tokens.is_empty() {
        groups.push(list_item_tokens);
    }

    groups
}

struct ResolveListState<'a> {
    tokens: Vec<&'a BlockToken>,
    spread: bool,
    next_item_index: usize,
    pending_item_index: Option<usize>,
    children: NodeBuffer,
}

struct ListParseGenerator<'a> {
    parse_api: &'a dyn ParseBlockPhaseApi,
    groups: Vec<Vec<&'a BlockToken>>,
    next_group_index: usize,
    current_list: Option<ResolveListState<'a>>,
    nodes: NodeBuffer,
    started: bool,
}

impl<'a> ParseBlockGenerator<'a> for ListParseGenerator<'a> {
    fn resume(
        &mut self,
        state: ParseBlockGeneratorResume,
    ) -> ParseBlockResult<ParseBlockGeneratorResult<'a>> {
        if let Some(current_list) = self.current_list.as_mut() {
            if let Some(item_index) = current_list.pending_item_index.take() {
                let nodes = match state {
                    ParseBlockGeneratorResume::Nodes(nodes) => nodes,
                    ParseBlockGeneratorResume::Error(error) => return Err(error),
                    ParseBlockGeneratorResume::Initial => {
                        return Err(ParseBlockError::new(
                            "[parseBlock] list resumed without child nodes",
                        ));
                    }
                };
                let list_item_token = current_list.tokens[item_index];
                let Some(data) = list_item_token.data_as::<ListTokenData>() else {
                    return Err(ParseBlockError::new(
                        "[parseBlock] list token data is missing",
                    ));
                };
                let children = if current_list.spread {
                    nodes
                } else {
                    flatten_paragraph_nodes(nodes)
                };
                current_list.children.push(Node::ListItem(ListItem {
                    position: if self.parse_api.should_reserve_position() {
                        list_item_token.position.clone()
                    } else {
                        None
                    },
                    status: data.status,
                    children,
                }));
                current_list.next_item_index = item_index + 1;
            } else {
                return Err(ParseBlockError::new(
                    "[parseBlock] list generator resumed unexpectedly",
                ));
            }
        } else if !self.started {
            if !matches!(state, ParseBlockGeneratorResume::Initial) {
                return Err(ParseBlockError::new(
                    "[parseBlock] list generator did not start from initial state",
                ));
            }
            self.started = true;
        } else {
            return Err(ParseBlockError::new(
                "[parseBlock] list generator resumed unexpectedly",
            ));
        }

        loop {
            if let Some(current_list) = self.current_list.as_mut() {
                if current_list.next_item_index < current_list.tokens.len() {
                    let item_index = current_list.next_item_index;
                    current_list.pending_item_index = Some(item_index);
                    return Ok(ParseBlockGeneratorResult::Yield(
                        self.parse_api
                            .request_block_tokens(Some(&current_list.tokens[item_index].children)),
                    ));
                }

                let current_list = self.current_list.take().expect("current list should exist");
                if let Some(node) = resolve_list(current_list, self.parse_api) {
                    self.nodes.push(node);
                }
                self.next_group_index += 1;
                continue;
            }

            let Some(tokens) = self.groups.get(self.next_group_index).cloned() else {
                return Ok(ParseBlockGeneratorResult::Complete(
                    std::mem::take(&mut self.nodes).into_vec(),
                ));
            };
            let spread = calc_spread(&tokens);
            let children = NodeBuffer::with_capacity(tokens.len());
            self.current_list = Some(ResolveListState {
                tokens,
                spread,
                next_item_index: 0,
                pending_item_index: None,
                children,
            });
        }
    }
}

fn resolve_list(state: ResolveListState<'_>, parse_api: &dyn ParseBlockPhaseApi) -> Option<Node> {
    let first_token = *state.tokens.first()?;
    let first_data = first_token.data_as::<ListTokenData>()?;

    let position = if parse_api.should_reserve_position() {
        calc_list_position(&state.tokens)
    } else {
        None
    };

    Some(Node::List(List {
        position,
        ordered: first_data.ordered,
        order_type: first_data.order_type.clone(),
        start: first_data.order,
        marker: first_data.marker,
        spread: state.spread,
        children: state.children.into_vec(),
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
