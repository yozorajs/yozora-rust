use yozora_ast::{Node, HEADING_TYPE};

use super::calc_last_source_line;
use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct HeadingWeaver;

impl NodeWeaver for HeadingWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![HEADING_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Heading(node) = node else {
            unreachable!()
        };
        if node.position.as_ref().map(|position| position.start.line)
            != calc_last_source_line(node.position.as_ref())
        {
            if node.depth == 1 {
                return NodeMarkup {
                    closer: Some("\n===".to_string()),
                    ..NodeMarkup::default()
                };
            }
            if node.depth == 2 {
                return NodeMarkup {
                    closer: Some("\n---".to_string()),
                    ..NodeMarkup::default()
                };
            }
        }
        NodeMarkup {
            opener: Some(format!("{} ", "#".repeat(usize::from(node.depth)))),
            ..NodeMarkup::default()
        }
    }
}
