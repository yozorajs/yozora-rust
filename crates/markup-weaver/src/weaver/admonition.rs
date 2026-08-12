use yozora_ast::{Node, ADMONITION_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct AdmonitionWeaver;

impl NodeWeaver for AdmonitionWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![ADMONITION_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave<'a>(
        &self,
        node: &'a Node,
        context: &NodeMarkupWeaveContext<'a>,
        _: usize,
    ) -> NodeMarkup {
        let Node::Admonition(node) = node else {
            unreachable!()
        };
        let title = context.weave_inline_nodes(&node.title);
        NodeMarkup {
            opener: Some(if title.is_empty() {
                format!(":::{}\n", node.keyword)
            } else {
                format!(":::{} {title}\n", node.keyword)
            }),
            closer: Some("\n:::".to_string()),
            ..NodeMarkup::default()
        }
    }
}
