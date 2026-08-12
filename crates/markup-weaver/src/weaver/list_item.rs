use yozora_ast::{Node, LIST_ITEM_TYPE};

use crate::util::minmax;
use crate::{Ancestor, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct ListItemWeaver;

impl NodeWeaver for ListItemWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![LIST_ITEM_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(
        &self,
        node: &Node,
        context: &NodeMarkupWeaveContext<'_>,
        child_index: usize,
    ) -> NodeMarkup {
        let Node::ListItem(node) = node else {
            unreachable!()
        };
        let Some(Ancestor::Node(Node::List(parent))) = context.ancestors().last() else {
            unreachable!()
        };
        let marker = char::from_u32(parent.marker).unwrap_or('-');
        let mut opener = format!("{marker} ");
        let mut indent = "  ".to_string();
        if parent.ordered {
            let order = parent.start.unwrap_or(1) as i64 + child_index as i64;
            let order_type = parent.order_type.as_deref().unwrap_or("1");
            let value = match order_type {
                "1" => order.max(0).to_string(),
                "a" => char::from_u32('a' as u32 + minmax(order - 1, 0, 25) as u32)
                    .unwrap_or('a')
                    .to_string(),
                "A" => char::from_u32('A' as u32 + minmax(order - 1, 0, 25) as u32)
                    .unwrap_or('A')
                    .to_string(),
                _ => order_type.to_string(),
            };
            opener = format!("{value}{marker} ");
            indent = " ".repeat(opener.len());
        }
        if let Some(status) = node.status {
            opener.push_str(match status {
                yozora_ast::TaskStatus::Todo => "[ ] ",
                yozora_ast::TaskStatus::Doing => "[-] ",
                yozora_ast::TaskStatus::Done => "[x] ",
            });
        }
        NodeMarkup {
            opener: Some(opener),
            indent: Some(indent),
            ..NodeMarkup::default()
        }
    }
}
