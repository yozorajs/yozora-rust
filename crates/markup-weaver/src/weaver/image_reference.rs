use yozora_ast::{Node, IMAGE_REFERENCE_TYPE};

use crate::util::create_character_escaper;
use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct ImageReferenceWeaver;

impl NodeWeaver for ImageReferenceWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![IMAGE_REFERENCE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::ImageReference(node) = node else {
            unreachable!()
        };
        NodeMarkup {
            opener: Some("![".to_string()),
            closer: Some(if node.alt == node.label {
                "][]".to_string()
            } else {
                format!("][{}]", node.label)
            }),
            content: Some(create_character_escaper(&['[', ']', '(', ')', '`'])(
                &node.alt,
            )),
            ..NodeMarkup::default()
        }
    }
}
