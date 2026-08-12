use yozora_ast::{Node, IMAGE_TYPE};

use crate::util::create_character_escaper;
use crate::{Escaper, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct ImageWeaver;

impl NodeWeaver for ImageWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![IMAGE_TYPE]
    }

    fn escape_content(&self) -> Option<Escaper> {
        Some(create_character_escaper(&['[', ']', '(', ')', '`']))
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Image(node) = node else {
            unreachable!()
        };
        let url = if node.url.contains(['(', ')']) {
            format!("<{}>", node.url)
        } else {
            node.url.clone()
        };
        let title = node
            .title
            .as_deref()
            .map(|title| create_character_escaper(&['"', '`'])(title));
        NodeMarkup {
            opener: Some("![".to_string()),
            closer: Some(match title {
                Some(title) => format!("]({url} \"{title}\")"),
                None => format!("]({url})"),
            }),
            content: Some(node.alt.clone()),
            ..NodeMarkup::default()
        }
    }
}
