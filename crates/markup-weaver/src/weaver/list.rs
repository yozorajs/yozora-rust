use yozora_ast::{Node, LIST_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct ListWeaver;

impl NodeWeaver for ListWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![LIST_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::List(node) = node else {
            unreachable!()
        };
        NodeMarkup {
            spread: Some(node.spread),
            ..NodeMarkup::default()
        }
    }
}
