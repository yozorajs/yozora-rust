use yozora_ast::{Node, STRONG_TYPE};

use crate::util::create_character_escaper;
use crate::{Escaper, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct StrongWeaver;

impl NodeWeaver for StrongWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![STRONG_TYPE]
    }

    fn escape_content(&self) -> Option<Escaper> {
        Some(create_character_escaper(&['*']))
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        NodeMarkup {
            opener: Some("**".to_string()),
            closer: Some("**".to_string()),
            ..NodeMarkup::default()
        }
    }
}
