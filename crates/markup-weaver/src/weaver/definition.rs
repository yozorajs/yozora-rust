use yozora_ast::{Node, DEFINITION_TYPE};

use crate::util::create_character_escaper;
use crate::{Escaper, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct DefinitionWeaver;

impl NodeWeaver for DefinitionWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![DEFINITION_TYPE]
    }

    fn escape_content(&self) -> Option<Escaper> {
        Some(create_character_escaper(&['[', ']', '(', ')']))
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Definition(node) = node else {
            unreachable!()
        };
        let url = if node.url.is_empty() { "<>" } else { &node.url };
        let title = node
            .title
            .as_deref()
            .map(|title| create_character_escaper(&['"'])(title));
        NodeMarkup {
            opener: Some("[".to_string()),
            closer: Some(match title {
                Some(title) => format!("]: {url} \"{title}\""),
                None => format!("]: {url}"),
            }),
            content: Some(node.label.clone()),
            ..NodeMarkup::default()
        }
    }
}
