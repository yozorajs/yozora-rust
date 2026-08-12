use yozora_ast::{Node, FOOTNOTE_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct FootnoteWeaver;

impl NodeWeaver for FootnoteWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![FOOTNOTE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        NodeMarkup {
            opener: Some("^[".to_string()),
            closer: Some("]".to_string()),
            ..NodeMarkup::default()
        }
    }
}
