use yozora_ast::{Node, PARAGRAPH_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct ParagraphWeaver;

impl NodeWeaver for ParagraphWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![PARAGRAPH_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        NodeMarkup::default()
    }
}
