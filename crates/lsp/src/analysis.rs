use std::cmp::Reverse;

use yozora_ast::{Definition, FootnoteDefinition, Node, Root};
use yozora_ast_util::{collect_definitions, collect_footnote_definitions};

use crate::protocol::{DocumentSymbol, FoldingRange, Position, Range};

struct HeadingSection {
    depth: u8,
    symbol: DocumentSymbol,
}

pub fn document_symbols(root: &Root) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    let mut stack: Vec<HeadingSection> = Vec::new();
    for section in heading_sections(root) {
        while stack.last().is_some_and(|parent| {
            parent.depth >= section.depth || parent.symbol.range.end < section.symbol.range.end
        }) {
            finish_symbol(&mut stack, &mut symbols);
        }
        stack.push(section);
    }
    while !stack.is_empty() {
        finish_symbol(&mut stack, &mut symbols);
    }
    symbols
}

fn finish_symbol(stack: &mut Vec<HeadingSection>, symbols: &mut Vec<DocumentSymbol>) {
    if let Some(section) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.symbol.children.push(section.symbol);
        } else {
            symbols.push(section.symbol);
        }
    }
}

fn heading_sections(root: &Root) -> Vec<HeadingSection> {
    let end = root
        .position
        .as_ref()
        .map(|position| position.end.into())
        .unwrap_or_default();
    let mut groups = vec![(root.children.as_slice(), end)];
    let mut sections: Vec<HeadingSection> = Vec::new();
    while let Some((nodes, scope_end)) = groups.pop() {
        let mut active: Vec<usize> = Vec::new();
        for node in nodes {
            let Some(position) = node.position() else {
                continue;
            };
            let range = Range::from(position);
            if let Node::Heading(heading) = node {
                // A heading closes preceding peers in the same block container.
                let boundary = Position {
                    line: range.start.line,
                    character: 0,
                };
                while active
                    .last()
                    .is_some_and(|&index| sections[index].depth >= heading.depth)
                {
                    let index = active.pop().expect("active heading exists");
                    let symbol = &mut sections[index].symbol;
                    symbol.range.end = boundary.max(symbol.selection_range.end);
                }
                let selection_range = match (
                    heading.children.first().and_then(Node::position),
                    heading.children.last().and_then(Node::position),
                ) {
                    (Some(first), Some(last)) => Range {
                        start: Position::from(first.start).max(range.start),
                        end: Position::from(last.end).min(range.end),
                    },
                    _ => Range {
                        start: range.start,
                        end: range.start,
                    },
                };
                active.push(sections.len());
                sections.push(HeadingSection {
                    depth: heading.depth,
                    symbol: DocumentSymbol {
                        name: heading_name(&heading.children),
                        kind: 15, // SymbolKind.String
                        range: Range {
                            start: range.start,
                            end: scope_end,
                        },
                        selection_range,
                        children: Vec::new(),
                    },
                });
            }
            if let Some(children) = node.children() {
                let children_end = children
                    .last()
                    .and_then(Node::position)
                    .map(|position| position.end.into())
                    .unwrap_or(range.end);
                groups.push((children, children_end.min(range.end).min(scope_end)));
            }
        }
    }
    sections.sort_by_key(|section| section.symbol.range.start);
    sections
}

fn heading_name(nodes: &[Node]) -> String {
    let name = plain_text(nodes);
    if name.is_empty() {
        "(empty heading)".to_string()
    } else {
        name
    }
}

fn plain_text(children: &[Node]) -> String {
    let mut text = String::new();
    for node in nodes(children) {
        if matches!(
            node,
            Node::Admonition(_)
                | Node::Blockquote(_)
                | Node::Code(_)
                | Node::Heading(_)
                | Node::ListItem(_)
                | Node::Math(_)
                | Node::Paragraph(_)
                | Node::TableCell(_)
        ) {
            text.push(' ');
        }
        match node {
            Node::Text(node) => text.push_str(&node.value),
            Node::InlineCode(node) => text.push_str(&node.value),
            Node::InlineMath(node) => text.push_str(&node.value),
            Node::Code(node) => text.push_str(&node.value),
            Node::Math(node) => text.push_str(&node.value),
            Node::Image(node) => text.push_str(&node.alt),
            Node::ImageReference(node) => text.push_str(&node.alt),
            Node::Break(_) => text.push(' '),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn folding_ranges(root: &Root) -> Vec<FoldingRange> {
    let mut ranges: Vec<_> = heading_sections(root)
        .into_iter()
        .map(|section| section.symbol.range)
        .collect();
    ranges.extend(nodes(&root.children).filter_map(|node| {
        if matches!(
            node,
            Node::Admonition(_)
                | Node::Blockquote(_)
                | Node::Code(_)
                | Node::Definition(_)
                | Node::FootnoteDefinition(_)
                | Node::Html(_)
                | Node::List(_)
                | Node::ListItem(_)
                | Node::Math(_)
                | Node::Table(_)
        ) {
            node.position().map(Range::from)
        } else {
            None
        }
    }));

    let mut folds: Vec<_> = ranges
        .into_iter()
        .filter_map(|range| {
            // AST ends are exclusive; folding ends are inclusive line numbers.
            let end_line = range
                .end
                .line
                .saturating_sub(u32::from(range.end.character == 0));
            (end_line > range.start.line).then_some(FoldingRange {
                start_line: range.start.line,
                end_line,
            })
        })
        .collect();
    folds.sort_by_key(|range| (range.start_line, Reverse(range.end_line)));
    // Editors can display one folding control per line; keep the outer range.
    folds.dedup_by_key(|range| range.start_line);
    folds
}

pub fn definition(root: &Root, position: Position) -> Option<Range> {
    let source = node_at(root, position, |node| reference_key(node).is_some())?;
    let (_, target) = resolve_symbol(root, source)?;
    target.range()
}

pub fn references(root: &Root, position: Position, include_declaration: bool) -> Vec<Range> {
    let Some(source) = node_at(root, position, |node| symbol_key(node).is_some()) else {
        return Vec::new();
    };
    let Some((key, target)) = resolve_symbol(root, source) else {
        return Vec::new();
    };
    let mut ranges: Vec<_> = nodes(&root.children)
        .filter(|node| reference_key(node) == Some(key))
        .filter_map(|node| node.position().map(Range::from))
        .collect();
    if include_declaration {
        ranges.extend(target.range());
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges.dedup();
    ranges
}

pub struct HoverInfo {
    pub text: String,
    pub range: Range,
}

pub fn hover(root: &Root, position: Position) -> Option<HoverInfo> {
    let source = node_at(root, position, |node| {
        matches!(node, Node::Link(_) | Node::Image(_)) || symbol_key(node).is_some()
    })?;
    let text = match source {
        Node::Link(link) => resource_hover(&link.url, link.title.as_deref()),
        Node::Image(image) => resource_hover(&image.url, image.title.as_deref()),
        _ => match resolve_symbol(root, source)?.1 {
            DefinitionTarget::Link(definition) => {
                resource_hover(&definition.url, definition.title.as_deref())
            }
            DefinitionTarget::Footnote(definition) => {
                let content = plain_text(&definition.children);
                format!("[^{}]\n\n{content}", definition.label)
                    .trim_end()
                    .to_string()
            }
        },
    };
    let mut characters = text.chars();
    let mut preview: String = characters.by_ref().take(2_000).collect();
    if characters.next().is_some() {
        preview.push('…');
    }
    Some(HoverInfo {
        text: preview,
        range: source.position().map(Range::from)?,
    })
}

fn resource_hover(url: &str, title: Option<&str>) -> String {
    match title.filter(|title| !title.trim().is_empty()) {
        Some(title) => format!("{url}\n\n{title}"),
        None => url.to_string(),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ReferenceKey<'a> {
    Link(&'a str),
    Footnote(&'a str),
}

pub(super) fn reference_key(node: &Node) -> Option<ReferenceKey<'_>> {
    match node {
        Node::LinkReference(reference) => Some(ReferenceKey::Link(&reference.identifier)),
        Node::ImageReference(reference) => Some(ReferenceKey::Link(&reference.identifier)),
        Node::FootnoteReference(reference) => Some(ReferenceKey::Footnote(&reference.identifier)),
        _ => None,
    }
}

pub(super) fn symbol_key(node: &Node) -> Option<ReferenceKey<'_>> {
    match node {
        Node::Definition(definition) => Some(ReferenceKey::Link(&definition.identifier)),
        Node::FootnoteDefinition(definition) => {
            Some(ReferenceKey::Footnote(&definition.identifier))
        }
        _ => reference_key(node),
    }
}

pub(super) enum DefinitionTarget<'a> {
    Link(&'a Definition),
    Footnote(&'a FootnoteDefinition),
}

impl DefinitionTarget<'_> {
    fn range(&self) -> Option<Range> {
        match self {
            Self::Link(definition) => definition.position.as_ref(),
            Self::Footnote(definition) => definition.position.as_ref(),
        }
        .map(Range::from)
    }
}

pub(super) fn resolve_symbol<'a>(
    root: &'a Root,
    source: &'a Node,
) -> Option<(ReferenceKey<'a>, DefinitionTarget<'a>)> {
    let key = symbol_key(source)?;
    // These collectors preserve the parser's first-definition-wins semantics.
    let target = match key {
        ReferenceKey::Link(identifier) => collect_definitions(root)
            .into_iter()
            .find(|definition| definition.identifier == identifier)
            .map(DefinitionTarget::Link)?,
        ReferenceKey::Footnote(identifier) => collect_footnote_definitions(root)
            .into_iter()
            .find(|definition| definition.identifier == identifier)
            .map(DefinitionTarget::Footnote)?,
    };
    // An inactive duplicate does not own the references bound to the first node.
    match (source, &target) {
        (Node::Definition(source), DefinitionTarget::Link(target))
            if !std::ptr::eq(source, *target) =>
        {
            return None;
        }
        (Node::FootnoteDefinition(source), DefinitionTarget::Footnote(target))
            if !std::ptr::eq(source, *target) =>
        {
            return None;
        }
        _ => {}
    }
    Some((key, target))
}

fn node_at(root: &Root, position: Position, is_target: impl Fn(&Node) -> bool) -> Option<&Node> {
    nodes(&root.children)
        .filter(|node| {
            node.position()
                .is_some_and(|range| Range::from(range).contains(position))
        })
        .filter(|node| is_target(node))
        .last()
}

pub(super) fn nodes(children: &[Node]) -> impl Iterator<Item = &Node> {
    let mut stack: Vec<_> = children.iter().rev().collect();
    std::iter::from_fn(move || {
        let node = stack.pop()?;
        if let Some(children) = node.children() {
            stack.extend(children.iter().rev());
        }
        // Admonition titles are inline content outside Node::children().
        if let Node::Admonition(admonition) = node {
            stack.extend(admonition.title.iter().rev());
        }
        Some(node)
    })
}

pub(super) fn single_line_label(label: &str) -> String {
    label
        .split([' ', '\t', '\n', '\r', '\u{000B}', '\u{000C}'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use yozora_core_parser::ParseOptions;
    use yozora_parser::YozoraParser;

    use super::*;

    fn parse(source: &str) -> Root {
        YozoraParser::default().parse(
            source,
            Some(ParseOptions {
                should_reserve_position: Some(true),
                ..ParseOptions::default()
            }),
        )
    }

    #[test]
    fn outlines_heading_sections_with_inline_text_and_utf16_ranges() {
        let root = parse("# 中文😀 *world*\r\nbody\r\n\r\n## Child\r\ntext\r\n# End");
        let symbols = document_symbols(&root);
        assert_eq!(symbols.len(), 2);
        let first = &symbols[0];
        assert_eq!(first.name, "中文😀 world");
        assert_eq!(
            first.range.start,
            Position {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            first.range.end,
            Position {
                line: 5,
                character: 0
            }
        );
        assert_eq!(first.selection_range.start.character, 2);
        assert_eq!(first.selection_range.end.character, 14);
        assert_eq!(first.children.len(), 1);
        assert_eq!(first.children[0].name, "Child");
        assert_eq!(first.children[0].range.end, first.range.end);
        assert_eq!(symbols[1].name, "End");
        assert_eq!(
            document_symbols(&parse("# foo*bar*baz"))[0].name,
            "foobarbaz"
        );
        assert_eq!(document_symbols(&parse("#"))[0].name, "(empty heading)");
    }

    #[test]
    fn folds_sections_and_blocks_without_swallowing_the_next_heading() {
        let root = parse("# One\ntext\n## Child\n```rs\nx\n```\n# Two\n");
        assert_eq!(
            folding_ranges(&root),
            vec![
                FoldingRange {
                    start_line: 0,
                    end_line: 5
                },
                FoldingRange {
                    start_line: 2,
                    end_line: 5
                },
                FoldingRange {
                    start_line: 3,
                    end_line: 5
                },
            ]
        );
        assert!(folding_ranges(&parse("# Empty\n# Also empty\n")).is_empty());
        assert_eq!(
            folding_ranges(&parse("Title\n=====\n")),
            vec![FoldingRange {
                start_line: 0,
                end_line: 1
            }]
        );
    }

    #[test]
    fn nested_headings_stop_at_their_block_container() {
        let root = parse("# Top\n> ## Quoted\n> body\n\noutside\n# Next\n");
        let symbols = document_symbols(&root);
        let quoted = &symbols[0].children[0];
        assert_eq!(quoted.name, "Quoted");
        assert_eq!(
            quoted.range.end,
            Position {
                line: 3,
                character: 0
            }
        );
        assert_eq!(
            folding_ranges(&root),
            vec![
                FoldingRange {
                    start_line: 0,
                    end_line: 4
                },
                FoldingRange {
                    start_line: 1,
                    end_line: 2
                },
            ]
        );
    }

    #[test]
    fn resolves_references_using_parser_normalization_and_first_definition() {
        let root = parse("😀 [go][FOO]\n\n[foo]: /first\n[foo]: /second\n\n![image][foo]\n\n[^n]: note\n\n[^n]\n");
        for position in [
            Position {
                line: 0,
                character: 5,
            },
            Position {
                line: 5,
                character: 4,
            },
        ] {
            assert_eq!(definition(&root, position).unwrap().start.line, 2);
        }
        assert_eq!(
            definition(
                &root,
                Position {
                    line: 9,
                    character: 2
                }
            )
            .unwrap()
            .start
            .line,
            7
        );
        assert!(definition(
            &root,
            Position {
                line: 0,
                character: 0
            }
        )
        .is_none());
        assert!(definition(
            &parse("[missing][]"),
            Position {
                line: 0,
                character: 2
            }
        )
        .is_none());
    }

    #[test]
    fn chooses_the_inner_reference_when_links_contain_reference_images() {
        let root = parse("[![alt][image]][link]\n\n[image]: /image\n[link]: /link\n");
        assert_eq!(
            definition(
                &root,
                Position {
                    line: 0,
                    character: 5
                }
            )
            .unwrap()
            .start
            .line,
            2
        );
        assert_eq!(
            definition(
                &root,
                Position {
                    line: 0,
                    character: 17
                }
            )
            .unwrap()
            .start
            .line,
            3
        );
    }

    #[test]
    fn references_share_link_labels_but_keep_footnotes_and_duplicates_separate() {
        let root = parse(concat!(
            "[Foo]: /first \"First\"\n",
            "[foo]: /unused\n\n",
            "[^foo]: note\n\n",
            "[^FOO]: ignored\n\n",
            "😀 [one][foo] ![two][FOO] [Foo][] [foo]\n",
            "[^FOO]\n\n",
            ":::note [title][foo] [^foo]\nbody\n:::\n",
        ));
        for position in [
            Position {
                line: 0,
                character: 2,
            },
            Position {
                line: 7,
                character: 4,
            },
            Position {
                line: 10,
                character: 10,
            },
        ] {
            let usages = references(&root, position, false);
            let starts: Vec<_> = usages
                .iter()
                .map(|range| (range.start.line, range.start.character))
                .collect();
            assert_eq!(starts, [(7, 3), (7, 14), (7, 26), (7, 34), (10, 8)]);
            let with_declaration = references(&root, position, true);
            assert_eq!(with_declaration.len(), 6);
            assert_eq!(with_declaration[0].start.line, 0);
        }
        for position in [
            Position {
                line: 3,
                character: 2,
            },
            Position {
                line: 8,
                character: 2,
            },
            Position {
                line: 10,
                character: 23,
            },
        ] {
            let lines: Vec<_> = references(&root, position, true)
                .iter()
                .map(|range| range.start.line)
                .collect();
            assert_eq!(lines, [3, 8, 10]);
        }
        for line in [1, 5] {
            let position = Position { line, character: 2 };
            assert!(references(&root, position, true).is_empty());
            assert!(hover(&root, position).is_none());
        }
    }

    #[test]
    fn hover_shows_resolved_resources_and_the_innermost_link() {
        let root = parse("😀 [go][FOO]\n\n[foo]: /first \"First\"\n[foo]: /unused\n");
        let info = hover(
            &root,
            Position {
                line: 0,
                character: 4,
            },
        )
        .unwrap();
        assert_eq!(info.text, "/first\n\nFirst");
        assert_eq!(
            info.range.start,
            Position {
                line: 0,
                character: 3
            }
        );

        let nested = parse("[![alt][image]][link]\n\n[image]: /image\n[link]: /link\n");
        let inner = Position {
            line: 0,
            character: 5,
        };
        let outer = Position {
            line: 0,
            character: 17,
        };
        assert_eq!(hover(&nested, inner).unwrap().text, "/image");
        assert_eq!(hover(&nested, outer).unwrap().text, "/link");
        assert_eq!(references(&nested, inner, false)[0].start.character, 1);
        assert_eq!(references(&nested, outer, false)[0].start.character, 0);

        for (source, expected) in [
            ("[link](/target \"Title\")", "/target\n\nTitle"),
            ("![image](/picture)", "/picture"),
            ("<https://example.com>", "https://example.com"),
        ] {
            let root = parse(source);
            let position = Position {
                line: 0,
                character: 2,
            };
            assert_eq!(hover(&root, position).unwrap().text, expected);
            assert!(references(&root, position, true).is_empty());
        }
    }

    #[test]
    fn footnote_hover_summarizes_blocks_and_bounds_unicode_previews() {
        let root = parse(concat!(
            "Use[^n]\n\n",
            "[^n]: first*bold*word\n\n",
            "    next paragraph with `code` and $x$.\n\n",
            "    ```text\n    code block\n    ```\n",
        ));
        let position = Position {
            line: 0,
            character: 5,
        };
        assert_eq!(
            hover(&root, position).unwrap().text,
            "[^n]\n\nfirstboldword next paragraph with code and x. code block"
        );
        let large = parse(&format!("Use[^n]\n\n[^n]: {}", "😀".repeat(2_500)));
        let preview = hover(&large, position).unwrap().text;
        assert_eq!(preview.chars().count(), 2_001);
        assert!(preview.ends_with("😀…"));
    }

    #[test]
    fn queries_return_no_symbol_for_unresolved_references_or_plain_text() {
        for source in [
            "plain text",
            "[missing][]",
            "[^missing]",
            "`[literal][id]`\n\n[id]: /url",
        ] {
            let root = parse(source);
            let position = Position {
                line: 0,
                character: 3,
            };
            assert!(hover(&root, position).is_none(), "{source}");
            assert!(references(&root, position, true).is_empty(), "{source}");
        }
    }
}
