use yozora_ast::{Node, FOOTNOTE_DEFINITION_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct FootnoteDefinitionWeaver;

impl NodeWeaver for FootnoteDefinitionWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![FOOTNOTE_DEFINITION_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::FootnoteDefinition(node) = node else {
            unreachable!()
        };
        NodeMarkup {
            opener: Some(format!("[^{}]: ", node.label)),
            indent: Some("    ".to_string()),
            ..NodeMarkup::default()
        }
    }
}
