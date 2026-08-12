use yozora_ast::{Node, LINK_REFERENCE_TYPE};

use crate::util::create_character_escaper;
use crate::{Escaper, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct LinkReferenceWeaver;

impl NodeWeaver for LinkReferenceWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![LINK_REFERENCE_TYPE]
    }

    fn escape_content(&self) -> Option<Escaper> {
        Some(create_character_escaper(&['[', ']', '(', ')', '`']))
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave<'a>(
        &self,
        node: &'a Node,
        context: &NodeMarkupWeaveContext<'a>,
        _: usize,
    ) -> NodeMarkup {
        let Node::LinkReference(node) = node else {
            unreachable!()
        };
        let content = context.weave_inline_nodes(&node.children);
        NodeMarkup {
            opener: Some("[".to_string()),
            closer: Some(if content == node.label {
                "][]".to_string()
            } else {
                format!("][{}]", node.label)
            }),
            ..NodeMarkup::default()
        }
    }
}
