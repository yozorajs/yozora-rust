use yozora_ast::{Node, BLOCKQUOTE_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct BlockquoteWeaver;

impl NodeWeaver for BlockquoteWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![BLOCKQUOTE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        NodeMarkup {
            opener: Some("> ".to_string()),
            indent: Some("> ".to_string()),
            ..NodeMarkup::default()
        }
    }
}
