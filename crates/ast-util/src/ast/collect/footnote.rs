use std::collections::HashSet;

use yozora_ast::{FootnoteDefinition, Node, Root, FOOTNOTE_DEFINITION_TYPE};

use crate::collect::collect_nodes;
use crate::NodeMatcher;

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
