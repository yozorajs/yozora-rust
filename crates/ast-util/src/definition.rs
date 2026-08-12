use std::collections::{HashMap, HashSet};

use yozora_ast::{Association, Definition, Node, Root};

use crate::{collect_definitions, traverse_ast, NodeMatcher};

pub struct DefinitionMapResult {
    pub root: Root,
    pub definition_map: HashMap<String, Definition>,
}

pub fn calc_identifier_set(
    root: &Root,
    matcher: NodeMatcher<'_>,
    preset_identifiers: &[Association],
) -> HashSet<String> {
    let mut identifiers = HashSet::new();
    traverse_ast(root, matcher, |node, _, _| {
        if let Node::Definition(definition) = node {
            identifiers.insert(definition.identifier.clone());
        }
    });
    identifiers.extend(
        preset_identifiers
            .iter()
            .map(|definition| definition.identifier.clone()),
    );
    identifiers
}

pub fn calc_definition_map(root: &Root, preset_definitions: &[Definition]) -> DefinitionMapResult {
    let mut definition_map = HashMap::new();
    for definition in collect_definitions(root) {
        definition_map
            .entry(definition.identifier.clone())
            .or_insert_with(|| definition.clone());
    }

    let mut additional = Vec::new();
    for definition in preset_definitions {
        if !definition_map.contains_key(&definition.identifier) {
            definition_map.insert(definition.identifier.clone(), definition.clone());
            additional.push(Node::Definition(definition.clone()));
        }
    }
    let mut root = root.clone();
    root.children.extend(additional);
    DefinitionMapResult {
        root,
        definition_map,
    }
}
