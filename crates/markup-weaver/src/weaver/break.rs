use yozora_ast::{Node, BREAK_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct BreakWeaver;

impl NodeWeaver for BreakWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![BREAK_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(
        &self,
        node: &Node,
        context: &NodeMarkupWeaveContext<'_>,
        child_index: usize,
    ) -> NodeMarkup {
        let parent = context.ancestors().last();
        let next = parent
            .filter(|parent| parent.children().get(child_index) == Some(node))
            .and_then(|parent| parent.children().get(child_index + 1));
        let has_legacy_line_ending =
            matches!(next, Some(Node::Text(text)) if text.value.starts_with(['\n', '\r']));
        NodeMarkup {
            opener: Some(if has_legacy_line_ending { "\\" } else { "\\\n" }.to_string()),
            ..NodeMarkup::default()
        }
    }
}
