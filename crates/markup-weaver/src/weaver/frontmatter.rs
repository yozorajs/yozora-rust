use yozora_ast::{Node, FRONTMATTER_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct FrontmatterWeaver;

impl NodeWeaver for FrontmatterWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![FRONTMATTER_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Frontmatter(node) = node else {
            unreachable!()
        };
        NodeMarkup {
            opener: Some("---\n".to_string()),
            closer: Some("\n---".to_string()),
            content: Some(node.value.clone()),
            ..NodeMarkup::default()
        }
    }
}
