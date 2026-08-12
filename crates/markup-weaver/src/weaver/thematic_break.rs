use yozora_ast::{Node, LIST_ITEM_TYPE, THEMATIC_BREAK_TYPE};

use crate::{Ancestor, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct ThematicBreakWeaver;

impl NodeWeaver for ThematicBreakWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![THEMATIC_BREAK_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, _: &Node, context: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let ancestors = context.ancestors();
        let opener = if ancestors.len() >= 2
            && ancestors
                .last()
                .is_some_and(|node| node.node_type() == LIST_ITEM_TYPE)
        {
            match ancestors[ancestors.len() - 2] {
                Ancestor::Node(Node::List(ref list)) if list.marker == '*' as u32 => "---",
                Ancestor::Node(Node::List(ref list)) if list.marker == '-' as u32 => "****",
                _ => "---",
            }
        } else {
            "---"
        };
        NodeMarkup {
            opener: Some(opener.to_string()),
            ..NodeMarkup::default()
        }
    }
}
