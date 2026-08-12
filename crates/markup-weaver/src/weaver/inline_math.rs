use yozora_ast::{Node, INLINE_MATH_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

#[derive(Debug, Clone, Copy, Default)]
pub struct InlineMathWeaverOptions {
    pub prefer_backtick: bool,
}

pub struct InlineMathWeaver {
    prefer_backtick: bool,
}

impl InlineMathWeaver {
    pub fn new(options: InlineMathWeaverOptions) -> Self {
        Self {
            prefer_backtick: options.prefer_backtick,
        }
    }
}

impl Default for InlineMathWeaver {
    fn default() -> Self {
        Self::new(InlineMathWeaverOptions::default())
    }
}

impl NodeWeaver for InlineMathWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![INLINE_MATH_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::InlineMath(node) = node else {
            unreachable!()
        };
        let value = node.value.trim();
        NodeMarkup {
            opener: Some(if self.prefer_backtick {
                format!("`${value}$`")
            } else {
                format!("${value}$")
            }),
            ..NodeMarkup::default()
        }
    }
}
