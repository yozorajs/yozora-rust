use std::cmp::Reverse;

use yozora_ast::{Node, Root};
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
    let mut text = String::new();
    let mut stack: Vec<_> = nodes.iter().rev().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Text(node) => text.push_str(&node.value),
            Node::InlineCode(node) => text.push_str(&node.value),
            Node::InlineMath(node) => text.push_str(&node.value),
            Node::Image(node) => text.push_str(&node.alt),
            Node::ImageReference(node) => text.push_str(&node.alt),
            Node::Break(_) => text.push(' '),
            _ => {
                if let Some(children) = node.children() {
                    stack.extend(children.iter().rev());
                }
            }
        }
    }
    let name = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        "(empty heading)".to_string()
    } else {
        name
    }
}

pub fn folding_ranges(root: &Root) -> Vec<FoldingRange> {
    let mut ranges: Vec<_> = heading_sections(root)
        .into_iter()
        .map(|section| section.symbol.range)
        .collect();
    ranges.extend(nodes(root).filter_map(|node| {
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
    enum Reference<'a> {
        Link(&'a str),
        Footnote(&'a str),
    }

    let reference = nodes(root)
        .filter(|node| {
            node.position()
                .is_some_and(|range| Range::from(range).contains(position))
        })
        .filter_map(|node| match node {
            Node::LinkReference(reference) => Some(Reference::Link(&reference.identifier)),
            Node::ImageReference(reference) => Some(Reference::Link(&reference.identifier)),
            Node::FootnoteReference(reference) => Some(Reference::Footnote(&reference.identifier)),
            _ => None,
        })
        .last()?;

    // These collectors preserve the parser's first-definition-wins semantics.
    match reference {
        Reference::Link(identifier) => collect_definitions(root)
            .into_iter()
            .find(|definition| definition.identifier == identifier)
            .and_then(|definition| definition.position.as_ref())
            .map(Range::from),
        Reference::Footnote(identifier) => collect_footnote_definitions(root)
            .into_iter()
            .find(|definition| definition.identifier == identifier)
            .and_then(|definition| definition.position.as_ref())
            .map(Range::from),
    }
}

fn nodes(root: &Root) -> impl Iterator<Item = &Node> {
    let mut stack: Vec<_> = root.children.iter().rev().collect();
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
}
