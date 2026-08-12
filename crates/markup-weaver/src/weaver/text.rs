use yozora_ast::{Node, TEXT_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct TextWeaver;

impl NodeWeaver for TextWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![TEXT_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Text(node) = node else {
            unreachable!()
        };
        let mut content = String::with_capacity(node.value.len());
        let characters = node.value.chars().collect::<Vec<_>>();
        for (index, character) in characters.iter().enumerate() {
            if *character == '\\' {
                let next = characters.get(index + 1).copied();
                if next.is_none_or(|value| {
                    value == '\r' || value == '\n' || value.is_ascii_punctuation()
                }) {
                    content.push('\\');
                }
            }
            content.push(*character);
        }
        NodeMarkup {
            content: Some(content),
            ..NodeMarkup::default()
        }
    }
}
