use yozora_ast::{List, ListItem, Node, Text};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::{ListItemToken, ListToken};

pub(crate) fn parse_list_token(token: ListToken) -> BlockTokenizeResult {
    let children = token.items.into_iter().map(build_item_node).collect();

    BlockTokenizeResult {
        node: Node::List(List {
            position: None,
            ordered: token.ordered,
            order_type: token.order_type,
            start: token.start,
            marker: token.marker,
            spread: token.spread,
            children,
        }),
        consumed_lines: token.consumed_lines,
    }
}

fn build_item_node(token: ListItemToken) -> Node {
    let children = if token.value.is_empty() {
        Vec::new()
    } else {
        vec![Node::Text(Text {
            position: None,
            value: token.value,
        })]
    };

    Node::ListItem(ListItem {
        position: None,
        status: token.status,
        children,
    })
}
