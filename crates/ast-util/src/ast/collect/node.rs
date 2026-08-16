use yozora_ast::{Node, Root};

use crate::NodeMatcher;

pub fn collect_nodes<'a>(root: &'a Root, matcher: NodeMatcher<'_>) -> Vec<&'a Node> {
    collect_nodes_from_slice(&root.children, matcher)
}

pub fn collect_nodes_from_node<'a>(root: &'a Node, matcher: NodeMatcher<'_>) -> Vec<&'a Node> {
    collect_nodes_from_slice(std::slice::from_ref(root), matcher)
}

fn collect_nodes_from_slice<'a>(nodes: &'a [Node], matcher: NodeMatcher<'_>) -> Vec<&'a Node> {
    let mut results = Vec::new();
    let mut stack = vec![(nodes, 0usize)];
    while let Some((nodes, index)) = stack.last_mut() {
        if *index >= nodes.len() {
            stack.pop();
            continue;
        }
        let node = &nodes[*index];
        *index += 1;
        if matcher.matches(node) {
            results.push(node);
            continue;
        }
        if let Some(children) = node.children().filter(|children| !children.is_empty()) {
            stack.push((children, 0));
        }
    }
    results
}

pub fn collect_root_nodes(root: &Root) -> Vec<&Node> {
    collect_nodes(root, NodeMatcher::All)
}
