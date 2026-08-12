use serde_json::Value;
use yozora_ast::{Node, Root};

use crate::NodeMatcher;

pub type UrlResolver = dyn for<'a> Fn(&'a [Option<&'a str>]) -> String;

pub fn default_url_resolver(path_pieces: &[Option<&str>]) -> String {
    let mut resolved_path = String::new();
    let mut suffix = String::new();
    for piece in path_pieces.iter().flatten() {
        let text = piece.trim();
        if text.is_empty() {
            continue;
        }
        let suffix_index = text.find(['?', '#']);
        let (path, next_suffix) = suffix_index.map_or((text, ""), |index| text.split_at(index));
        if !path.is_empty() {
            if resolved_path.is_empty() || path.starts_with('/') || has_uri_prefix(path) {
                resolved_path = path.to_string();
            } else {
                resolved_path.push('/');
                resolved_path.push_str(path);
            }
            suffix = next_suffix.to_string();
        } else if next_suffix.starts_with('?') {
            suffix = next_suffix.to_string();
        } else {
            if let Some(index) = suffix.find('#') {
                suffix.truncate(index);
            }
            suffix.push_str(next_suffix);
        }
    }
    format!("{}{suffix}", normalize_url_path(&resolved_path))
}

pub fn resolve_urls_for_ast<F>(ast: &mut Root, matcher: NodeMatcher<'_>, mut resolve_url: F)
where
    F: FnMut(&str) -> String,
{
    fn visit<F>(nodes: &mut [Node], matcher: &NodeMatcher<'_>, resolve_url: &mut F)
    where
        F: FnMut(&str) -> String,
    {
        for node in nodes {
            if matcher.matches(node) {
                match node {
                    Node::Definition(node) => node.url = resolve_url(&node.url),
                    Node::Image(node) => node.url = resolve_url(&node.url),
                    Node::Link(node) => node.url = resolve_url(&node.url),
                    Node::Custom(node) => {
                        if let Some(url) = node.data.get("url").and_then(Value::as_str) {
                            node.data
                                .insert("url".to_string(), Value::String(resolve_url(url)));
                        }
                    }
                    _ => {}
                }
            }
            if let Node::Admonition(node) = node {
                visit(&mut node.children, matcher, resolve_url);
                visit(&mut node.title, matcher, resolve_url);
                continue;
            }
            match node {
                Node::Blockquote(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Delete(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Emphasis(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Footnote(node) => visit(&mut node.children, matcher, resolve_url),
                Node::FootnoteDefinition(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Heading(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Link(node) => visit(&mut node.children, matcher, resolve_url),
                Node::LinkReference(node) => visit(&mut node.children, matcher, resolve_url),
                Node::List(node) => visit(&mut node.children, matcher, resolve_url),
                Node::ListItem(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Paragraph(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Strong(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Table(node) => visit(&mut node.children, matcher, resolve_url),
                Node::TableRow(node) => visit(&mut node.children, matcher, resolve_url),
                Node::TableCell(node) => visit(&mut node.children, matcher, resolve_url),
                Node::Custom(node) => {
                    if let Some(children) = node.children.as_mut() {
                        visit(children, matcher, resolve_url);
                    }
                }
                _ => {}
            }
        }
    }
    visit(&mut ast.children, &matcher, &mut resolve_url);
}

pub fn resolve_all_urls_for_ast<F>(ast: &mut Root, resolve_url: F)
where
    F: FnMut(&str) -> String,
{
    resolve_urls_for_ast(
        ast,
        NodeMatcher::Predicate(&|node| {
            matches!(node, Node::Definition(_) | Node::Image(_) | Node::Link(_))
                || matches!(node, Node::Custom(node) if node.data.contains_key("url"))
        }),
        resolve_url,
    );
}

fn normalize_url_path(path: &str) -> String {
    let (prefix, pathname, opaque) = split_url_prefix(path);
    if opaque || pathname.is_empty() {
        return path.to_string();
    }
    let absolute = pathname.starts_with('/');
    let preserve_trailing =
        pathname.ends_with('/') || pathname.ends_with("/.") || pathname.ends_with("/..");
    let mut segments: Vec<&str> = Vec::new();
    for segment in pathname.split('/') {
        match segment {
            "" | "." => {}
            ".." if segments.last().is_some_and(|last| *last != "..") => {
                segments.pop();
            }
            ".." if !absolute => segments.push(segment),
            ".." => {}
            _ => segments.push(segment),
        }
    }
    let mut normalized = format!("{}{}", if absolute { "/" } else { "" }, segments.join("/"));
    if preserve_trailing && !normalized.is_empty() && !normalized.ends_with('/') {
        normalized.push('/');
    }
    format!("{prefix}{normalized}")
}

fn has_uri_prefix(path: &str) -> bool {
    !split_url_prefix(path).0.is_empty()
}

fn split_url_prefix(path: &str) -> (&str, &str, bool) {
    if let Some(rest) = path.strip_prefix("//") {
        let authority_end = rest.find('/').map_or(path.len(), |index| index + 2);
        return (&path[..authority_end], &path[authority_end..], false);
    }
    let Some(colon) = path.find(':') else {
        return ("", path, false);
    };
    let scheme = &path[..colon];
    if scheme.is_empty()
        || !scheme.starts_with(|character: char| character.is_ascii_alphabetic())
        || !scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '.' | '-')
        })
    {
        return ("", path, false);
    }
    if path[colon + 1..].starts_with("//") {
        let authority_start = colon + 3;
        let authority_end = path[authority_start..]
            .find('/')
            .map_or(path.len(), |index| authority_start + index);
        (&path[..authority_end], &path[authority_end..], false)
    } else {
        (&path[..=colon], &path[colon + 1..], true)
    }
}
