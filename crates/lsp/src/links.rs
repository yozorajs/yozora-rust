use std::collections::HashMap;

use yozora_ast::{Node, Root};
use yozora_ast_util::{calc_heading_identifiers, collect_definitions};

use crate::analysis::{heading_selection_range, nodes, resource_or_symbol_at};
use crate::protocol::{DocumentLink, Position, Range};

const MAX_DOCUMENT_LINKS: usize = 1_000;
const MAX_DOCUMENT_LINK_BYTES: usize = 1024 * 1024;

pub fn heading(root: &Root, fragment: &str, prefix: &str) -> Option<Range> {
    headings(root, prefix)
        .find(|(identifier, _)| identifier == fragment)
        .map(|(_, range)| range)
}

pub fn headings<'a>(root: &'a Root, prefix: &str) -> impl Iterator<Item = (String, Range)> + 'a {
    let headings = root.children.iter().filter_map(|node| match node {
        Node::Heading(heading) => Some(heading),
        _ => None,
    });
    headings
        .zip(calc_heading_identifiers(root, prefix))
        .filter_map(|(heading, identifier)| {
            heading.position.as_ref().map(|position| {
                (
                    identifier,
                    heading_selection_range(heading, Range::from(position)),
                )
            })
        })
}

fn direct_destination(node: &Node) -> Option<&str> {
    match node {
        Node::Link(link) => Some(&link.url),
        Node::Image(image) => Some(&image.url),
        Node::Definition(definition) => Some(&definition.url),
        _ => None,
    }
}

pub fn destination_at(root: &Root, position: Position) -> Option<&str> {
    resource_or_symbol_at(root, position).and_then(direct_destination)
}

pub fn document_links(
    root: &Root,
    mut resolve: impl FnMut(&str) -> Option<String>,
) -> Vec<DocumentLink> {
    let definitions: HashMap<_, _> = collect_definitions(root)
        .into_iter()
        .map(|definition| (definition.identifier.as_str(), definition.url.as_str()))
        .collect();
    let resources = nodes(&root.children)
        .filter_map(|node| {
            let destination = direct_destination(node).or_else(|| match node {
                Node::LinkReference(reference) => {
                    definitions.get(reference.identifier.as_str()).copied()
                }
                Node::ImageReference(reference) => {
                    definitions.get(reference.identifier.as_str()).copied()
                }
                _ => None,
            })?;
            Some((node.position().map(Range::from)?, destination))
        })
        // Bound filesystem probes as well as output, even if many links fail.
        .take(MAX_DOCUMENT_LINKS);
    let mut budget = MAX_DOCUMENT_LINK_BYTES;
    let mut links = Vec::new();
    for (range, destination) in resources {
        // A single long definition can be repeated by thousands of references.
        // Bound both resolver input and URI output, not just the link count.
        if budget == 0 || destination.len() > budget {
            break;
        }
        budget -= destination.len();
        let Some(target) = resolve(destination) else {
            continue;
        };
        if target.len() > budget {
            break;
        }
        budget -= target.len();
        links.push(DocumentLink { range, target });
    }
    links
}

#[cfg(test)]
mod tests {
    use yozora_core_parser::ParseOptions;
    use yozora_parser::YozoraParser;

    use super::*;

    fn parse(text: &str) -> Root {
        YozoraParser::default().parse(
            text,
            Some(ParseOptions {
                should_reserve_position: Some(true),
                ..ParseOptions::default()
            }),
        )
    }

    #[test]
    fn anchors_follow_toc_ids_without_changing_the_ast() {
        let root = parse("# Intro\n# Intro-2\n# Intro\n# foo*bar*baz\n# 中文😀\n\n> # Intro\n\n# Intro\n\nTitle\n=====\n\n#\n");
        let original = root.clone();
        for (identifier, line, character) in [
            ("intro", 0, 2),
            ("intro-2", 1, 2),
            ("intro-3", 2, 2),
            ("foo-bar-baz", 3, 2),
            ("中文😀", 4, 2),
            ("intro-4", 8, 2),
            ("title", 10, 0),
            ("", 13, 0),
        ] {
            let range = heading(&root, identifier, "").unwrap();
            assert_eq!(range.start, Position { line, character }, "{identifier}");
            assert_eq!(
                heading(&root, &format!("h-{identifier}"), "h-"),
                Some(range)
            );
        }
        assert_eq!(heading(&root, "中文😀", "").unwrap().end.character, 6);
        for identifier in ["Intro", "foobarbaz", "intro-5", "%E4%B8%AD%E6%96%87"] {
            assert!(heading(&root, identifier, "").is_none(), "{identifier}");
        }
        assert_eq!(root, original);
    }

    #[test]
    fn chooses_inner_resources_and_uses_ast_ranges() {
        let root =
            parse("[![alt](image.md)](guide.md)\n\n[key]: target.md\n\n`[code](ignored.md)`\n");
        assert_eq!(
            destination_at(
                &root,
                Position {
                    line: 0,
                    character: 4
                }
            ),
            Some("image.md")
        );
        assert_eq!(
            destination_at(
                &root,
                Position {
                    line: 0,
                    character: 20
                }
            ),
            Some("guide.md")
        );
        assert_eq!(
            destination_at(
                &root,
                Position {
                    line: 2,
                    character: 9
                }
            ),
            Some("target.md")
        );
        assert_eq!(
            destination_at(
                &root,
                Position {
                    line: 4,
                    character: 5
                }
            ),
            None
        );
        let links = document_links(&root, |url| Some(url.to_string()));
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].range.start.character, 0);
        assert_eq!(links[1].range.start.character, 1);
        assert!(links[0].range.end > links[1].range.end);
    }

    #[test]
    fn document_links_resolve_reference_namespaces_once_and_bound_probes() {
        let root = parse("[go][LABEL]\n\n![alt][label]\n\n[label]: first.md\n[label]: second.md\n\n[^label]: [nested](note.md)\n\n[^label]\n");
        let links = document_links(&root, |url| Some(url.to_string()));
        assert_eq!(
            links
                .iter()
                .map(|link| link.target.as_str())
                .collect::<Vec<_>>(),
            ["first.md", "first.md", "first.md", "second.md", "note.md"]
        );
        let root = parse(&"[go](target.md)\n\n".repeat(1_500));
        let mut probes = 0;
        assert!(document_links(&root, |_| {
            probes += 1;
            None
        })
        .is_empty());
        assert_eq!(probes, 1_000);
        assert_eq!(
            document_links(&root, |url| Some(url.to_string())).len(),
            1_000
        );
    }

    #[test]
    fn bounds_repeated_long_destinations_before_expanding_them() {
        let destination = format!("https://example.test/{}", "a".repeat(65_536));
        let root = parse(&format!(
            "{}\n[ref]: {destination}",
            "[go][ref]\n\n".repeat(128)
        ));
        let mut processed_bytes = 0;
        let links = document_links(&root, |url| {
            processed_bytes += url.len();
            Some(url.to_string())
        });
        assert!(!links.is_empty());
        assert!(links.len() < 16);
        assert!(processed_bytes <= MAX_DOCUMENT_LINK_BYTES);
        assert!(
            links.iter().map(|link| link.target.len()).sum::<usize>() <= MAX_DOCUMENT_LINK_BYTES
        );

        let oversized = parse(&format!(
            "[go](https://example.test/{})",
            "a".repeat(MAX_DOCUMENT_LINK_BYTES)
        ));
        assert!(document_links(&oversized, |_| panic!(
            "oversized destination must not reach the resolver"
        ))
        .is_empty());
        let root = parse("[go](#heading) [again](#heading)");
        let mut probes = 0;
        assert!(document_links(&root, |_| {
            probes += 1;
            Some("a".repeat(MAX_DOCUMENT_LINK_BYTES + 1))
        })
        .is_empty());
        assert_eq!(
            probes, 1,
            "stop when a long base URI exhausts the output budget"
        );
    }
}
