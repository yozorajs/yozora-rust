use yozora_ast::{Node, MATH_TYPE};

use crate::util::find_max_continuous_symbol;
use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

#[derive(Debug, Clone, Copy, Default)]
pub struct MathWeaverOptions {
    pub prefer_backtick: bool,
}

pub struct MathWeaver;

impl MathWeaver {
    pub fn new(_: MathWeaverOptions) -> Self {
        Self
    }
}

impl NodeWeaver for MathWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![MATH_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Math(node) = node else {
            unreachable!()
        };
        let value = node.value.trim();
        let count = find_max_continuous_symbol(value, '$');
        let markers = "$".repeat(if count == 0 { 2 } else { count + 1 });
        let opener = if value.contains(['\r', '\n']) {
            format!("{markers}\n{value}\n{markers}")
        } else if count == 0 {
            format!("{markers}{value}{markers}")
        } else {
            format!("{markers} {value} {markers}")
        };
        NodeMarkup {
            opener: Some(opener),
            ..NodeMarkup::default()
        }
    }
}
