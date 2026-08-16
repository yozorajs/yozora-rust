use std::collections::HashSet;

use yozora_ast::{Definition, Node, Root, DEFINITION_TYPE};

use crate::collect::collect_nodes;
use crate::NodeMatcher;

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
