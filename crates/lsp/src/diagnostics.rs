use std::collections::{hash_map::Entry, HashMap};

use yozora_ast::{Node, Root, DEFINITION_TYPE, FOOTNOTE_DEFINITION_TYPE};
use yozora_ast_util::{collect_nodes, NodeMatcher};

use crate::protocol::Range;

pub const MAX_DIAGNOSTICS: usize = 1_000;

#[derive(Debug)]
pub struct DuplicateDefinition {
    pub range: Range,
    pub first_definition: Range,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn duplicate_definitions(root: &Root) -> Vec<DuplicateDefinition> {
    let mut diagnostics = Vec::new();
    for (node_type, code, message) in [
        (
            DEFINITION_TYPE,
            "duplicate-link-definition",
            "Duplicate link definition; references use the first definition.",
        ),
        (
            FOOTNOTE_DEFINITION_TYPE,
            "duplicate-footnote-definition",
            "Duplicate footnote definition; references use the first definition.",
        ),
    ] {
        let mut first_definitions = HashMap::new();
        // Match the per-namespace traversal of the definition collectors, but
        // retain duplicates. In particular, footnote bodies are not searched
        // for more footnote declarations by reference resolution.
        diagnostics.extend(
            collect_nodes(root, NodeMatcher::Types(&[node_type]))
                .into_iter()
                .filter_map(|node| {
                    let identifier = match node {
                        Node::Definition(definition) => definition.identifier.as_str(),
                        Node::FootnoteDefinition(definition) => definition.identifier.as_str(),
                        _ => return None,
                    };
                    let range = Range::from(node.position()?);
                    match first_definitions.entry(identifier) {
                        Entry::Vacant(entry) => {
                            entry.insert(range);
                            None
                        }
                        Entry::Occupied(entry) => Some(DuplicateDefinition {
                            range,
                            first_definition: *entry.get(),
                            code,
                            message,
                        }),
                    }
                })
                // Keeping this many per namespace is sufficient to return the
                // earliest MAX_DIAGNOSTICS after merging in source order.
                .take(MAX_DIAGNOSTICS),
        );
    }
    diagnostics.sort_by_key(|diagnostic| (diagnostic.range.start, diagnostic.range.end));
    diagnostics.truncate(MAX_DIAGNOSTICS);
    diagnostics
}

#[cfg(test)]
mod tests {
    use yozora_core_parser::ParseOptions;
    use yozora_parser::YozoraParser;

    use super::*;
    use crate::protocol::Position;

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
    fn follows_parser_case_folding_with_utf16_ranges_and_separate_namespaces() {
        let root = parse("[Straße😀]: /first\r\n[STRASSE😀]: /second\r\n[strasse😀]: /third\r\n\r\n[^Straße😀]: first\r\n\r\n[^STRASSE😀]: second\r\n");
        let diagnostics = duplicate_definitions(&root);
        assert_eq!(diagnostics.len(), 3);
        assert_eq!(
            diagnostics[0].range,
            Range {
                start: Position {
                    line: 1,
                    character: 0
                },
                end: Position {
                    line: 2,
                    character: 0
                },
            }
        );
        for diagnostic in &diagnostics[..2] {
            assert_eq!(diagnostic.code, "duplicate-link-definition");
            assert_eq!(diagnostic.first_definition.start.line, 0);
            assert_eq!(
                diagnostic.first_definition.end,
                Position {
                    line: 1,
                    character: 0
                }
            );
        }
        assert_eq!(diagnostics[1].range.start.line, 2);
        assert_eq!(diagnostics[2].code, "duplicate-footnote-definition");
        assert_eq!(diagnostics[2].range.start.line, 6);
        assert_eq!(diagnostics[2].first_definition.start.line, 4);
        let final_line = duplicate_definitions(&parse("[中😀]: /first\r\n[中😀]: /second"));
        assert_eq!(
            final_line[0].range.end,
            Position {
                line: 1,
                character: 14
            }
        );
    }

    #[test]
    fn finds_multiline_escaped_labels_across_block_containers() {
        let root = parse(
            "> [A\\]B\n> label]: /first\n>\n> [a\\]b\tLABEL]: /second\n\n:::note\n[A\\]B LABEL]: /third\n:::\n",
        );
        let diagnostics = duplicate_definitions(&root);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics[0].range.start,
            Position {
                line: 3,
                character: 2
            }
        );
        assert_eq!(
            diagnostics[1].range.start,
            Position {
                line: 6,
                character: 0
            }
        );
        for diagnostic in diagnostics {
            assert_eq!(
                diagnostic.first_definition.start,
                Position {
                    line: 0,
                    character: 2
                }
            );
            assert_eq!(
                diagnostic.first_definition.end,
                Position {
                    line: 2,
                    character: 0
                }
            );
        }
    }

    #[test]
    fn keeps_the_same_footnote_declaration_scope_as_navigation() {
        let root = parse("[^outer]: container\n    [^inner]: nested\n\n[^inner]: first\n\n[^INNER]: second\n\n[^inner]\n");
        let diagnostics = duplicate_definitions(&root);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].range.start.line, 5);
        assert_eq!(diagnostics[0].first_definition.start.line, 3);
        assert_eq!(
            crate::analysis::definition(
                &root,
                Position {
                    line: 7,
                    character: 3
                }
            ),
            Some(diagnostics[0].first_definition)
        );
    }

    #[test]
    fn leaves_unresolved_references_and_definition_like_literal_text_alone() {
        let root = parse("[missing][] [^missing] ![alt][absent]\n\n[real]: /url\n[other]: /url\n\n```md\n[real]: /ignored\n[^n]: first\n[^n]: second\n```\n\n$$\n[real]: /ignored\n$$\n\n<div>\n[real]: /ignored\n</div>\n\n`[real]: /ignored`\n\n\\[real]: /ignored\n");
        assert!(duplicate_definitions(&root).is_empty());
    }

    #[test]
    fn bounds_diagnostics_in_source_order_across_namespaces() {
        let mut text = String::from("[^n]: first\n\n");
        for _ in 0..MAX_DIAGNOSTICS + 5 {
            text.push_str("[^n]: duplicate\n\n");
        }
        text.push_str("[ref]: /first\n");
        for _ in 0..MAX_DIAGNOSTICS + 5 {
            text.push_str("[ref]: /duplicate\n");
        }
        let diagnostics = duplicate_definitions(&parse(&text));
        assert_eq!(diagnostics.len(), MAX_DIAGNOSTICS);
        for (index, diagnostic) in diagnostics.iter().enumerate() {
            assert_eq!(diagnostic.code, "duplicate-footnote-definition");
            assert_eq!(diagnostic.range.start.line, (index as u32 + 1) * 2);
            assert_eq!(diagnostic.first_definition.start.line, 0);
        }
    }
}
