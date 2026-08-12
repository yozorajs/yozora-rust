use yozora_ast::{Node, INLINE_CODE_TYPE};

use crate::util::find_max_continuous_symbol;
use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct InlineCodeWeaver;

impl NodeWeaver for InlineCodeWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![INLINE_CODE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::InlineCode(node) = node else {
            unreachable!()
        };
        let count = find_max_continuous_symbol(&node.value, '`');
        let opener = if count == 0 {
            format!("`{}`", node.value)
        } else {
            let markers = "`".repeat(count + 1);
            format!("{markers} {} {markers}", node.value)
        };
        NodeMarkup {
            opener: Some(opener),
            ..NodeMarkup::default()
        }
    }
}
