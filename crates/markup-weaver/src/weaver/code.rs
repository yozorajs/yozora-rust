use yozora_ast::{Node, CODE_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct CodeWeaver;

impl NodeWeaver for CodeWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![CODE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Code(node) = node else {
            unreachable!()
        };
        let mut info_string = String::new();
        if let Some(language) = node.lang.as_deref() {
            info_string.push_str(language);
            if let Some(meta) = node.meta.as_deref() {
                info_string.push(' ');
                info_string.push_str(meta);
            }
        }
        let marker = if info_string.contains('`') { '~' } else { '`' };
        let count = node
            .value
            .lines()
            .map(|line| {
                line.chars()
                    .take_while(|character| *character == marker)
                    .count()
            })
            .max()
            .unwrap_or(0);
        let markers = marker
            .to_string()
            .repeat(if count == 0 { 3 } else { count + 1 });
        NodeMarkup {
            opener: Some(format!("{markers}{info_string}\n{}{markers}", node.value)),
            ..NodeMarkup::default()
        }
    }
}
