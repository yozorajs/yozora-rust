use yozora_ast::{Node, FOOTNOTE_REFERENCE_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct FootnoteReferenceWeaver;

impl NodeWeaver for FootnoteReferenceWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![FOOTNOTE_REFERENCE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::FootnoteReference(node) = node else {
            unreachable!()
        };
        NodeMarkup {
            opener: Some("[^".to_string()),
            closer: Some("]".to_string()),
            content: Some(node.label.clone()),
            ..NodeMarkup::default()
        }
    }
}
