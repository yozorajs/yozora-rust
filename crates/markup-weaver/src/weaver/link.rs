use yozora_ast::{Node, LINK_TYPE};

use crate::util::create_character_escaper;
use crate::{Escaper, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct LinkWeaver;

impl NodeWeaver for LinkWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![LINK_TYPE]
    }

    fn escape_content(&self) -> Option<Escaper> {
        Some(create_character_escaper(&['[', ']', '(', ')', '`']))
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Link(node) = node else {
            unreachable!()
        };
        let is_protocol_autolink = node.title.is_none()
            && node.children.len() == 1
            && matches!(&node.children[0], Node::Text(text) if text.value == node.url)
            && (node.url.starts_with("mailto:") || node.url.starts_with("xmpp:"));
        if is_protocol_autolink {
            return NodeMarkup {
                content: Some(node.url.clone()),
                ..NodeMarkup::default()
            };
        }

        let is_encoded_autolink = node.title.is_none()
            && node.children.len() == 1
            && matches!(
                &node.children[0],
                Node::Text(text) if text.value.replace(']', "%5D") == node.url
            );
        let url = if is_encoded_autolink {
            node.url.replace('(', "\\(").replace(')', "\\)")
        } else if node.url.contains(['(', ')']) {
            format!("<{}>", node.url)
        } else {
            node.url.clone()
        };
        let title = node
            .title
            .as_deref()
            .map(|title| create_character_escaper(&['"', '`'])(title));
        NodeMarkup {
            opener: Some("[".to_string()),
            closer: Some(match title {
                Some(title) => format!("]({url} \"{title}\")"),
                None => format!("]({url})"),
            }),
            ..NodeMarkup::default()
        }
    }
}
