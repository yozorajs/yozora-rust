use std::sync::Arc;

use yozora_ast::{Node, ROOT_TYPE};

use crate::{Escaper, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct RootWeaver;

impl NodeWeaver for RootWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![ROOT_TYPE]
    }

    fn escape_content(&self) -> Option<Escaper> {
        Some(Arc::new(escape_content))
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        NodeMarkup::default()
    }
}

fn escape_content(content: &str) -> String {
    let mut result = content
        .split('\n')
        .enumerate()
        .map(|(index, line)| escape_line(line, index > 0))
        .collect::<Vec<_>>()
        .join("\n");
    let trailing_backslashes = result
        .chars()
        .rev()
        .take_while(|value| *value == '\\')
        .count();
    if trailing_backslashes % 2 == 1 {
        result.push('\\');
    }
    result
}

fn escape_line(line: &str, follows_line_ending: bool) -> String {
    let leading = line
        .char_indices()
        .find(|(_, character)| *character != ' ' && *character != '\t')
        .map_or(line.len(), |(index, _)| index);
    let rest = &line[leading..];
    let should_escape = rest.starts_with('>')
        || heading_marker(rest)
        || setext_marker(rest)
        || (follows_line_ending && (list_marker(rest) || thematic_break_marker(rest)));
    let mut line = if should_escape {
        format!("{}\\{rest}", &line[..leading])
    } else {
        line.to_string()
    };
    if let Some(index) = closing_heading_marker(&line) {
        line.insert(index, '\\');
    }
    line
}

fn heading_marker(value: &str) -> bool {
    let count = value
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (1..=6).contains(&count)
        && value
            .chars()
            .nth(count)
            .is_none_or(|character| character == ' ' || character == '\t')
}

fn setext_marker(value: &str) -> bool {
    let value = value.trim_end_matches([' ', '\t']);
    value.len() >= 3 && value.chars().all(|character| character == '=')
}

fn list_marker(value: &str) -> bool {
    matches!(value.as_bytes(), [b'-' | b'*' | b'+', b' ' | b'\t', _, ..])
}

fn thematic_break_marker(value: &str) -> bool {
    let mut marker = None;
    let mut count = 0;
    for character in value.chars() {
        if character == ' ' || character == '\t' {
            continue;
        }
        if !matches!(character, '-' | '_' | '*') {
            return false;
        }
        if marker.is_some_and(|marker| marker != character) {
            return false;
        }
        marker = Some(character);
        count += 1;
    }
    count >= 3
}

fn closing_heading_marker(value: &str) -> Option<usize> {
    let trimmed = value.trim_end_matches(['\r', '\n']);
    let hash_start = trimmed.trim_end_matches('#').len();
    if hash_start == trimmed.len() || hash_start == 0 {
        return None;
    }
    trimmed[..hash_start]
        .chars()
        .last()
        .is_some_and(|character| character == ' ' || character == '\t')
        .then_some(hash_start)
}
