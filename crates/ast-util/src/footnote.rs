use std::collections::HashMap;

use yozora_ast::{FootnoteDefinition, FootnoteReference, Node, Paragraph, Root, FOOTNOTE_TYPE};

use crate::tree::{clone_with_children, clone_with_title};
use crate::{collect_footnote_definitions, collect_nodes, NodeMatcher};

pub const DEFAULT_FOOTNOTE_IDENTIFIER_PREFIX: &str = "footnote-";

pub struct FootnoteDefinitionMapResult {
    pub root: Root,
    pub footnote_definition_map: HashMap<String, FootnoteDefinition>,
}

pub fn calc_footnote_definition_map(
    root: &Root,
    preset_definitions: &[FootnoteDefinition],
    prefer_references: bool,
    identifier_prefix: &str,
) -> FootnoteDefinitionMapResult {
    let mut footnote_definition_map = HashMap::new();
    for definition in collect_footnote_definitions(root) {
        footnote_definition_map
            .entry(definition.identifier.clone())
            .or_insert_with(|| definition.clone());
    }

    let mut root = root.clone();
    for definition in preset_definitions {
        if !footnote_definition_map.contains_key(&definition.identifier) {
            footnote_definition_map.insert(definition.identifier.clone(), definition.clone());
            root.children
                .push(Node::FootnoteDefinition(definition.clone()));
        }
    }

    if prefer_references {
        root =
            replace_footnotes_in_references(&root, &mut footnote_definition_map, identifier_prefix);
        footnote_definition_map.clear();
        for definition in collect_footnote_definitions(&root) {
            footnote_definition_map
                .entry(definition.identifier.clone())
                .or_insert_with(|| definition.clone());
        }
    }

    FootnoteDefinitionMapResult {
        root,
        footnote_definition_map,
    }
}

pub fn replace_footnotes_in_references(
    root: &Root,
    footnote_definition_map: &mut HashMap<String, FootnoteDefinition>,
    identifier_prefix: &str,
) -> Root {
    if collect_nodes(root, NodeMatcher::Types(&[FOOTNOTE_TYPE])).is_empty()
        && !contains_footnote_in_admonition_title(&root.children)
    {
        return crate::shallow_clone_ast(root, |_, _, _| false);
    }

    enum Field {
        Title,
        Children,
    }
    struct Frame<'a> {
        node: &'a Node,
        fields: Vec<(Field, &'a [Node])>,
        field_index: usize,
        child_index: usize,
        traversed: Vec<Node>,
        output: Vec<Node>,
        title: Option<Vec<Node>>,
        children: Option<Vec<Node>>,
    }

    fn frame(node: &Node) -> Frame<'_> {
        let mut fields = Vec::new();
        if let Node::Admonition(admonition) = node {
            fields.push((Field::Title, admonition.title.as_slice()));
        }
        if let Some(children) = node.children() {
            fields.push((Field::Children, children));
        }
        Frame {
            node,
            fields,
            field_index: 0,
            child_index: 0,
            traversed: Vec::new(),
            output: Vec::new(),
            title: None,
            children: None,
        }
    }

    fn next_identifier(
        footnote_id: &mut usize,
        definitions: &HashMap<String, FootnoteDefinition>,
        prefix: &str,
    ) -> (String, String) {
        loop {
            let label = footnote_id.to_string();
            *footnote_id += 1;
            let identifier = format!("{prefix}{label}");
            if !definitions.contains_key(&label) && !definitions.contains_key(&identifier) {
                return (label, identifier);
            }
        }
    }

    fn replace_footnote(
        node: Node,
        footnote_id: &mut usize,
        footnote_definition_map: &mut HashMap<String, FootnoteDefinition>,
        identifier_prefix: &str,
        new_definitions: &mut Vec<Node>,
    ) -> Node {
        let Node::Footnote(footnote) = node else {
            return node;
        };
        let (label, identifier) =
            next_identifier(footnote_id, footnote_definition_map, identifier_prefix);
        let definition = FootnoteDefinition {
            position: None,
            identifier: identifier.clone(),
            label: label.clone(),
            children: vec![Node::Paragraph(Paragraph {
                position: None,
                children: footnote.children,
            })],
        };
        let reference = Node::FootnoteReference(FootnoteReference {
            position: footnote.position,
            identifier: identifier.clone(),
            label,
        });
        footnote_definition_map.insert(identifier, definition.clone());
        new_definitions.push(Node::FootnoteDefinition(definition));
        reference
    }

    let mut root_output = Vec::new();
    let mut root_traversed = Vec::new();
    let mut root_index = 0usize;
    let mut stack = Vec::<Frame<'_>>::new();
    let mut footnote_id = 1usize;
    let mut new_definitions = Vec::new();
    loop {
        if let Some(current) = stack.last_mut() {
            if current.field_index < current.fields.len() {
                let nodes = current.fields[current.field_index].1;
                if current.child_index < nodes.len() {
                    let child = &nodes[current.child_index];
                    current.child_index += 1;
                    stack.push(frame(child));
                    continue;
                }

                for node in std::mem::take(&mut current.traversed) {
                    current.output.push(replace_footnote(
                        node,
                        &mut footnote_id,
                        footnote_definition_map,
                        identifier_prefix,
                        &mut new_definitions,
                    ));
                }

                let output = std::mem::take(&mut current.output);
                match current.fields[current.field_index].0 {
                    Field::Title => current.title = Some(output),
                    Field::Children => current.children = Some(output),
                }
                current.field_index += 1;
                current.child_index = 0;
                continue;
            }

            let current = stack.pop().expect("footnote frame should exist");
            let mut node = current.node.clone();
            if let Some(title) = current.title {
                node = clone_with_title(&node, title);
            }
            if let Some(children) = current.children {
                node = clone_with_children(&node, children);
            }
            if let Some(parent) = stack.last_mut() {
                parent.traversed.push(node);
            } else {
                root_traversed.push(node);
            }
            continue;
        }

        if root_index < root.children.len() {
            stack.push(frame(&root.children[root_index]));
            root_index += 1;
            continue;
        }
        for node in root_traversed {
            root_output.push(replace_footnote(
                node,
                &mut footnote_id,
                footnote_definition_map,
                identifier_prefix,
                &mut new_definitions,
            ));
        }
        root_output.extend(new_definitions);
        return Root {
            node_type: root.node_type.clone(),
            position: root.position.clone(),
            children: root_output,
        };
    }
}

fn contains_footnote_in_admonition_title(nodes: &[Node]) -> bool {
    let mut stack = vec![(nodes, 0usize)];
    while let Some((nodes, index)) = stack.last_mut() {
        if *index >= nodes.len() {
            stack.pop();
            continue;
        }
        let node = &nodes[*index];
        *index += 1;
        if let Node::Admonition(admonition) = node {
            if admonition
                .title
                .iter()
                .any(|node| node.node_type() == FOOTNOTE_TYPE)
            {
                return true;
            }
            stack.push((&admonition.title, 0));
        }
        if let Some(children) = node.children().filter(|children| !children.is_empty()) {
            stack.push((children, 0));
        }
    }
    false
}
