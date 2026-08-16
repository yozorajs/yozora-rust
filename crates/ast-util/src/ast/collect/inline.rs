use yozora_ast::{Node, Root};

use crate::collect::collect_nodes;
use crate::NodeMatcher;

pub fn collect_inline_nodes(root: &Root) -> Vec<&Node> {
    collect_nodes(root, NodeMatcher::Predicate(&inline_node_matcher))
}

pub fn inline_node_matcher(node: &Node) -> bool {
    const INLINE_TYPES: &[&str] = &[
        "break",
        "delete",
        "emphasis",
        "footnoteReference",
        "footnote",
        "imageReference",
        "image",
        "inlineCode",
        "inlineMath",
        "linkReference",
        "link",
        "strong",
        "text",
    ];
    INLINE_TYPES.contains(&node.node_type())
}
