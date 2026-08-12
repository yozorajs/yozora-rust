use yozora_ast::{Node, EMPHASIS_TYPE};

use crate::util::create_character_escaper;
use crate::{Escaper, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct EmphasisWeaver;

impl NodeWeaver for EmphasisWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![EMPHASIS_TYPE]
    }

    fn escape_content(&self) -> Option<Escaper> {
        Some(create_character_escaper(&['*', '_']))
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, _: &Node, context: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let mut bit = 0;
        for ancestor in context.ancestors().iter().rev() {
            if ancestor.node_type() == EMPHASIS_TYPE {
                bit ^= 1;
            } else {
                break;
            }
        }
        let symbol = if bit == 0 { "*" } else { "_" };
        NodeMarkup {
            opener: Some(symbol.to_string()),
            closer: Some(symbol.to_string()),
            ..NodeMarkup::default()
        }
    }
}
