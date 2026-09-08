use std::collections::HashMap;

use yozora_ast::{Node, Root};
use yozora_character::{fold_case, is_punctuation_character};

use crate::collect_texts;

#[derive(Debug, Clone, PartialEq)]
pub struct HeadingToc {
    pub children: Vec<HeadingTocNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeadingTocNode {
    pub identifier: String,
    pub depth: u8,
    pub contents: Vec<Node>,
    pub children: Vec<HeadingTocNode>,
}

/// IDs for top-level headings in source order, without changing the AST.
/// Nested headings are excluded, matching `calc_heading_toc`.
pub fn calc_heading_identifiers(ast: &Root, identifier_prefix: &str) -> Vec<String> {
    let mut next_suffixes = HashMap::<String, usize>::new();
    let mut identifiers = Vec::new();
    for node in &ast.children {
        let Node::Heading(heading) = node else {
            continue;
        };
        let base = format!(
            "{identifier_prefix}{}",
            calc_identifier_from_nodes(&heading.children)
        );
        let mut identifier = base.clone();
        if let Some(mut suffix) = next_suffixes.get(&base).copied() {
            loop {
                identifier = format!("{base}-{suffix}");
                suffix += 1;
                if !next_suffixes.contains_key(&identifier) {
                    break;
                }
            }
            next_suffixes.insert(base, suffix);
        }
        next_suffixes.insert(identifier.clone(), 2);
        identifiers.push(identifier);
    }
    identifiers
}

pub fn calc_heading_toc(ast: &mut Root, identifier_prefix: &str) -> HeadingToc {
    let identifiers = calc_heading_identifiers(ast, identifier_prefix);
    let headings = ast.children.iter_mut().filter_map(|node| match node {
        Node::Heading(heading) => Some(heading),
        _ => None,
    });
    let mut flat_nodes = Vec::<HeadingTocNode>::new();
    for (heading, identifier) in headings.zip(identifiers) {
        heading.identifier = Some(identifier.clone());
        flat_nodes.push(HeadingTocNode {
            identifier,
            depth: heading.depth,
            contents: heading.children.clone(),
            children: Vec::new(),
        });
    }

    let mut roots = Vec::new();
    let mut paths: Vec<usize> = Vec::new();
    for node in flat_nodes {
        while let Some(parent) = get_node(&roots, &paths) {
            if parent.depth < node.depth {
                break;
            }
            paths.pop();
        }
        let siblings = get_children_mut(&mut roots, &paths);
        siblings.push(node);
        paths.push(siblings.len() - 1);
    }
    HeadingToc { children: roots }
}

pub fn calc_identifier_from_nodes(nodes: &[Node]) -> String {
    let content = collect_texts(nodes).join("-").trim().to_lowercase();
    let mut identifier = String::new();
    let mut pending_dash = false;
    for character in content.chars() {
        if character.is_whitespace() || is_punctuation_character(character as i32) {
            pending_dash = !identifier.is_empty();
        } else {
            if pending_dash {
                identifier.push('-');
                pending_dash = false;
            }
            identifier.push(character);
        }
    }
    fold_case(identifier.trim_matches('-'))
}

fn get_node<'a>(roots: &'a [HeadingTocNode], path: &[usize]) -> Option<&'a HeadingTocNode> {
    let mut nodes = roots;
    let mut current = None;
    for index in path {
        current = nodes.get(*index);
        nodes = &current?.children;
    }
    current
}

fn get_children_mut<'a>(
    roots: &'a mut Vec<HeadingTocNode>,
    path: &[usize],
) -> &'a mut Vec<HeadingTocNode> {
    let mut nodes = roots;
    for index in path {
        nodes = &mut nodes[*index].children;
    }
    nodes
}
