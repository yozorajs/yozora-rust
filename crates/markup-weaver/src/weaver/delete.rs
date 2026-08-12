use yozora_ast::{Node, DELETE_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct DeleteWeaver;

impl NodeWeaver for DeleteWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![DELETE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        NodeMarkup {
            opener: Some("~~".to_string()),
            closer: Some("~~".to_string()),
            ..NodeMarkup::default()
        }
    }
}
