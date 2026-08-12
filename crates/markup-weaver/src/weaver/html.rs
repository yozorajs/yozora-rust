use yozora_ast::{Node, HTML_TYPE, PARAGRAPH_TYPE};

use super::calc_last_source_line;
use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct HtmlWeaver;

impl NodeWeaver for HtmlWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![HTML_TYPE]
    }

    fn is_block_level(
        &self,
        node: &Node,
        context: &NodeMarkupWeaveContext<'_>,
        child_index: usize,
    ) -> bool {
        if context
            .ancestors()
            .iter()
            .any(|ancestor| ancestor.node_type() == PARAGRAPH_TYPE)
        {
            return false;
        }
        let Some(parent) = context.ancestors().last() else {
            return true;
        };
        if child_index > 0 {
            let previous = &parent.children()[child_index - 1];
            if calc_last_source_line(previous.position())
                == node.position().map(|position| position.start.line)
            {
                return false;
            }
        }
        if let Some(next) = parent.children().get(child_index + 1) {
            if calc_last_source_line(node.position())
                == next.position().map(|position| position.start.line)
            {
                return false;
            }
        }
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Html(node) = node else {
            unreachable!()
        };
        NodeMarkup {
            opener: Some(node.value.clone()),
            ..NodeMarkup::default()
        }
    }
}
