use std::collections::HashSet;

use yozora_ast::{
    Definition, FootnoteDefinition, Node, Root, DEFINITION_TYPE, FOOTNOTE_DEFINITION_TYPE,
};

use crate::NodeMatcher;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShallowNode<T> {
    One(T),
    Many(Vec<T>),
    None,
}

pub struct ShallowNodeCollector<T> {
    nodes: Vec<T>,
    next_nodes: Option<Vec<T>>,
}

impl<T> ShallowNodeCollector<T>
where
    T: Clone + PartialEq,
{
    pub fn add(&mut self, node: ShallowNode<T>, original_node: &T, original_index: usize) {
        if matches!(&node, ShallowNode::One(next_node) if next_node == original_node) {
            if let (Some(next_nodes), ShallowNode::One(node)) = (&mut self.next_nodes, node) {
                next_nodes.push(node);
            }
            return;
        }

        let next_nodes = self
            .next_nodes
            .get_or_insert_with(|| self.nodes[..original_index.min(self.nodes.len())].to_vec());
        match node {
            ShallowNode::One(node) => next_nodes.push(node),
            ShallowNode::Many(nodes) => next_nodes.extend(nodes),
            ShallowNode::None => {}
        }
    }

    pub fn collect(self) -> Vec<T> {
        self.next_nodes.unwrap_or(self.nodes)
    }
}

pub fn create_shallow_node_collector<T>(nodes: Vec<T>) -> ShallowNodeCollector<T> {
    ShallowNodeCollector {
        nodes,
        next_nodes: None,
    }
}

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

pub fn collect_texts(nodes: &[Node]) -> Vec<String> {
    let mut texts = Vec::new();
    let mut stack = vec![(nodes, 0usize)];
    while let Some((nodes, index)) = stack.last_mut() {
        if *index >= nodes.len() {
            stack.pop();
            continue;
        }
        let node = &nodes[*index];
        *index += 1;
        let text = match node {
            Node::Code(node) => Some(node.value.as_str()),
            Node::Frontmatter(node) => Some(node.value.as_str()),
            Node::Html(node) => Some(node.value.as_str()),
            Node::Image(node) => Some(node.alt.as_str()),
            Node::ImageReference(node) => Some(node.alt.as_str()),
            Node::InlineCode(node) => Some(node.value.as_str()),
            Node::InlineMath(node) => Some(node.value.as_str()),
            Node::Math(node) => Some(node.value.as_str()),
            Node::Text(node) => Some(node.value.as_str()),
            _ => None,
        };
        if let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) {
            texts.push(text.to_string());
        } else if let Some(children) = node.children().filter(|children| !children.is_empty()) {
            stack.push((children, 0));
        }
    }
    texts
}

pub fn collect_definitions(root: &Root) -> Vec<&Definition> {
    let nodes = collect_nodes(root, NodeMatcher::Types(&[DEFINITION_TYPE]));
    let mut identifiers = HashSet::new();
    nodes
        .into_iter()
        .filter_map(|node| match node {
            Node::Definition(definition) if identifiers.insert(definition.identifier.clone()) => {
                Some(definition)
            }
            _ => None,
        })
        .collect()
}

pub fn collect_footnote_definitions(root: &Root) -> Vec<&FootnoteDefinition> {
    let nodes = collect_nodes(root, NodeMatcher::Types(&[FOOTNOTE_DEFINITION_TYPE]));
    let mut identifiers = HashSet::new();
    nodes
        .into_iter()
        .filter_map(|node| match node {
            Node::FootnoteDefinition(definition)
                if identifiers.insert(definition.identifier.clone()) =>
            {
                Some(definition)
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{create_shallow_node_collector, ShallowNode};

    #[test]
    fn shallow_collector_reuses_or_changes_sequence() {
        let nodes = vec![1, 2, 3];
        let mut unchanged = create_shallow_node_collector(nodes.clone());
        for (index, node) in nodes.iter().copied().enumerate() {
            unchanged.add(ShallowNode::One(node), &node, index);
        }
        assert_eq!(unchanged.collect(), nodes);

        let mut changed = create_shallow_node_collector(vec![1, 2, 3]);
        changed.add(ShallowNode::One(1), &1, 0);
        changed.add(ShallowNode::Many(vec![4, 5]), &2, 1);
        changed.add(ShallowNode::None, &3, 2);
        assert_eq!(changed.collect(), vec![1, 4, 5]);
    }
}
