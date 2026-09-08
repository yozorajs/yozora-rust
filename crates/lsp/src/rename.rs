use std::ops::Range as ByteRange;

use serde_json::{json, Value};
use yozora_ast::{Node, ReferenceType, Root};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::analysis::{
    nodes, reference_key, resolve_symbol, single_line_label, symbol_key, DefinitionTarget,
    ReferenceKey,
};
use crate::cancellation::Cancellation;
use crate::document::{check_size, Snapshot};
use crate::protocol::{Position, PrepareRenameResult, Range, ResponseError, TextEdit};

enum EditKind {
    Label,
    Collapsed,
    Shortcut,
}

struct Occurrence {
    label: ByteRange<usize>,
    edit: ByteRange<usize>,
    kind: EditKind,
}

struct Subject<'a> {
    key: ReferenceKey<'a>,
    label: &'a str,
    range: Range,
}

pub fn prepare(snapshot: &Snapshot<'_>, position: Position) -> Option<PrepareRenameResult> {
    let subject = subject_at(snapshot, position)?;
    Some(PrepareRenameResult {
        range: subject.range,
        placeholder: single_line_label(subject.label),
    })
}

pub fn rename(
    snapshot: &Snapshot<'_>,
    position: Position,
    new_name: &str,
    parser: &YozoraParser,
    cancellation: &Cancellation,
) -> Result<Vec<TextEdit>, ResponseError> {
    cancellation.check()?;
    let subject = subject_at(snapshot, position)
        .ok_or_else(|| failed("position is not a resolved reference label"))?;
    let new_identifier = validate_name(subject.key, new_name, parser)?;
    cancellation.check()?;
    let new_key = match subject.key {
        ReferenceKey::Link(_) => ReferenceKey::Link(&new_identifier),
        ReferenceKey::Footnote(_) => ReferenceKey::Footnote(&new_identifier),
    };
    if subject.key != new_key
        && nodes(&snapshot.root.children).any(|node| {
            matches!(node, Node::Definition(_) | Node::FootnoteDefinition(_))
                && symbol_key(node) == Some(new_key)
        })
    {
        return Err(failed("the new label is already defined in this namespace"));
    }
    if single_line_label(subject.label) == new_name {
        return Ok(Vec::new());
    }

    let mut replacements = Vec::new();
    let mut edited_length = snapshot.text.len();
    for node in nodes(&snapshot.root.children).filter(|node| symbol_key(node) == Some(subject.key))
    {
        cancellation.check()?;
        let occurrence = occurrence(snapshot, node)
            .ok_or_else(|| failed("cannot locate every occurrence of this label"))?;
        let new_text = match occurrence.kind {
            EditKind::Label => new_name.to_string(),
            EditKind::Collapsed if subject.key != new_key => new_name.to_string(),
            EditKind::Shortcut if subject.key != new_key => format!("[{new_name}]"),
            _ => continue,
        };
        if snapshot.text[occurrence.edit.clone()] != new_text {
            edited_length = edited_length - occurrence.edit.len() + new_text.len();
            check_size(edited_length)?;
            replacements.push((occurrence.edit, new_text));
        }
    }
    cancellation.check()?;
    replacements.sort_by_key(|(range, _)| (range.start, range.end));
    if replacements
        .windows(2)
        .any(|pair| pair[0].0.end > pair[1].0.start)
    {
        return Err(failed("reference edits overlap"));
    }
    let mut edited = String::with_capacity(edited_length);
    let mut cursor = 0;
    for (range, new_text) in &replacements {
        cancellation.check()?;
        edited.push_str(&snapshot.text[cursor..range.start]);
        edited.push_str(new_text);
        cursor = range.end;
    }
    edited.push_str(&snapshot.text[cursor..]);

    // Validate the complete edit before offering it: a new name can turn plain
    // Markdown into a reference, or interfere with table and container syntax.
    cancellation.check()?;
    let reparsed = parser.parse(
        &edited,
        Some(ParseOptions {
            should_reserve_position: Some(true),
            ..ParseOptions::default()
        }),
    );
    cancellation.check()?;
    if !preserves_semantics(snapshot.root, &reparsed, subject.key, new_key) {
        return Err(failed(
            "rename would change other Markdown syntax or reference bindings",
        ));
    }
    replacements
        .into_iter()
        .map(|(range, new_text)| {
            cancellation.check()?;
            Ok(TextEdit {
                range: source_range(snapshot, range)
                    .ok_or_else(|| failed("invalid source range"))?,
                new_text,
            })
        })
        .collect()
}

fn subject_at<'a>(snapshot: &Snapshot<'a>, position: Position) -> Option<Subject<'a>> {
    nodes(&snapshot.root.children)
        .filter(|node| {
            node.position().is_some_and(|range| {
                let range = Range::from(range);
                range.start <= position && position <= range.end
            })
        })
        .filter_map(|node| {
            let occurrence = occurrence(snapshot, node)?;
            let range = source_range(snapshot, occurrence.label)?;
            let edit_range = source_range(snapshot, occurrence.edit)?;
            let range = if range.start <= position && position <= range.end {
                range
            } else if matches!(occurrence.kind, EditKind::Collapsed) && position == edit_range.start
            {
                edit_range
            } else {
                return None;
            };
            let (key, target) = resolve_symbol(snapshot.root, node)?;
            let label = match target {
                DefinitionTarget::Link(definition) => definition.label.as_str(),
                DefinitionTarget::Footnote(definition) => definition.label.as_str(),
            };
            Some(Subject { key, label, range })
        })
        .last()
}

fn occurrence(snapshot: &Snapshot<'_>, node: &Node) -> Option<Occurrence> {
    symbol_key(node)?;
    let position = node.position()?;
    let start = snapshot
        .lines
        .byte_offset(snapshot.text, position.start.into())
        .ok()?;
    let end = snapshot
        .lines
        .byte_offset(snapshot.text, position.end.into())
        .ok()?;
    let text = snapshot.text.get(start..end)?;

    let (label, edit, kind) = match node {
        Node::Definition(_) | Node::FootnoteDefinition(_) => {
            let opening = text.find('[')?;
            let marker_width = if matches!(node, Node::FootnoteDefinition(_)) {
                2
            } else {
                1
            };
            if marker_width == 2 && text.as_bytes().get(opening + 1) != Some(&b'^') {
                return None;
            }
            let label_start = opening + marker_width;
            let closing = closing_bracket(text, label_start)?;
            if text.as_bytes().get(closing + 1) != Some(&b':') {
                return None;
            }
            let label = label_start..closing;
            (label.clone(), label, EditKind::Label)
        }
        Node::FootnoteReference(_) => {
            if !text.starts_with("[^") || !text.ends_with(']') {
                return None;
            }
            let label = 2..text.len() - 1;
            (label.clone(), label, EditKind::Label)
        }
        Node::LinkReference(reference) => {
            reference_occurrence(text, reference.reference_type, false)?
        }
        Node::ImageReference(reference) => {
            reference_occurrence(text, reference.reference_type, true)?
        }
        _ => return None,
    };
    Some(Occurrence {
        label: start + label.start..start + label.end,
        edit: start + edit.start..start + edit.end,
        kind,
    })
}

fn reference_occurrence(
    text: &str,
    reference_type: ReferenceType,
    image: bool,
) -> Option<(ByteRange<usize>, ByteRange<usize>, EditKind)> {
    let marker = if image { "![" } else { "[" };
    if !text.starts_with(marker) || !text.ends_with(']') {
        return None;
    }
    let end = text.len() - 1;
    match reference_type {
        ReferenceType::Full => {
            let opening = text[..end]
                .char_indices()
                .rev()
                .find(|(index, character)| {
                    *character == '['
                        && text.as_bytes()[..*index]
                            .iter()
                            .rev()
                            .take_while(|byte| **byte == b'\\')
                            .count()
                            % 2
                            == 0
                })?
                .0;
            let label = opening + 1..end;
            Some((label.clone(), label, EditKind::Label))
        }
        ReferenceType::Collapsed if text.ends_with("][]") => {
            Some((marker.len()..text.len() - 3, end..end, EditKind::Collapsed))
        }
        ReferenceType::Shortcut => Some((
            marker.len()..end,
            text.len()..text.len(),
            EditKind::Shortcut,
        )),
        _ => None,
    }
}

fn closing_bracket(text: &str, start: usize) -> Option<usize> {
    let mut escaped = false;
    for (index, character) in text.get(start..)?.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '[' => return None,
            ']' => return Some(start + index),
            _ => {}
        }
    }
    None
}

fn source_range(snapshot: &Snapshot<'_>, range: ByteRange<usize>) -> Option<Range> {
    Some(Range {
        start: snapshot.lines.position(snapshot.text, range.start).ok()?,
        end: snapshot.lines.position(snapshot.text, range.end).ok()?,
    })
}

fn validate_name(
    key: ReferenceKey<'_>,
    name: &str,
    parser: &YozoraParser,
) -> Result<String, ResponseError> {
    if name.is_empty()
        || name.trim() != name
        || name.chars().count() > 999
        || name
            .chars()
            .any(|character| character.is_control() && character != '\t')
    {
        return Err(ResponseError::invalid_params("new name must be a nonempty, single-line Markdown label of at most 999 characters, without surrounding whitespace"));
    }
    let probe = match key {
        ReferenceKey::Link(_) => format!("[{name}]: /rename-target"),
        ReferenceKey::Footnote(_) => format!("[^{name}]: rename target"),
    };
    let parsed = parser.parse(&probe, None);
    if parsed.children.len() == 1 {
        match (key, &parsed.children[0]) {
            (ReferenceKey::Link(_), Node::Definition(definition)) if definition.label == name => {
                return Ok(definition.identifier.clone());
            }
            (ReferenceKey::Footnote(_), Node::FootnoteDefinition(definition))
                if definition.label == name =>
            {
                return Ok(definition.identifier.clone());
            }
            _ => {}
        }
    }
    Err(ResponseError::invalid_params(
        "new name is not a valid label in this reference namespace",
    ))
}

fn preserves_semantics(
    before: &Root,
    after: &Root,
    old: ReferenceKey<'_>,
    new: ReferenceKey<'_>,
) -> bool {
    if before.children.len() != after.children.len() {
        return false;
    }
    let mut left = nodes(&before.children);
    let mut right = nodes(&after.children);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return true,
            (Some(left), Some(right)) => {
                let affected = symbol_key(left) == Some(old);
                let expected_key = if affected {
                    Some(new)
                } else {
                    symbol_key(left)
                };
                if symbol_key(right) != expected_key {
                    return false;
                }
                let (Some(left), Some(right)) =
                    (signature(left, affected), signature(right, affected))
                else {
                    return false;
                };
                if left != right {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

// Compare shallow payloads in source order. This avoids cloning or serializing
// a recursive AST and keeps checks safe for deeply nested documents.
fn signature(node: &Node, affected: bool) -> Option<Value> {
    let mut value = match node {
        Node::Admonition(node) => {
            json!({ "keyword": node.keyword, "titleCount": node.title.len() })
        }
        Node::Heading(node) => json!({ "depth": node.depth, "identifier": node.identifier }),
        Node::Link(node) => json!({ "url": node.url, "title": node.title }),
        Node::LinkReference(node) => {
            json!({ "identifier": node.identifier, "label": node.label, "referenceType": node.reference_type })
        }
        Node::FootnoteDefinition(node) => {
            json!({ "identifier": node.identifier, "label": node.label })
        }
        Node::List(node) => {
            json!({ "ordered": node.ordered, "orderType": node.order_type, "start": node.start, "marker": node.marker, "spread": node.spread })
        }
        Node::ListItem(node) => json!({ "status": node.status }),
        Node::Table(node) => json!({ "columns": node.columns }),
        Node::Blockquote(_)
        | Node::Delete(_)
        | Node::Emphasis(_)
        | Node::Footnote(_)
        | Node::Paragraph(_)
        | Node::Strong(_)
        | Node::TableRow(_)
        | Node::TableCell(_) => json!({}),
        Node::Custom(_) => return None,
        _ if node.children().is_none() => serde_json::to_value(node).ok()?,
        _ => return None,
    };
    let object = value.as_object_mut()?;
    object.remove("position");
    object.insert("type".into(), Value::String(node.node_type().to_string()));
    if let Some(children) = node.children() {
        object.insert("childrenCount".into(), json!(children.len()));
    }
    if affected {
        object.remove("identifier");
        object.remove("label");
        if reference_key(node).is_some() {
            object.remove("referenceType");
        }
    }
    Some(value)
}

fn failed(message: &str) -> ResponseError {
    ResponseError::new(-32803, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::protocol::ContentChange;

    fn marked_document(marked: &str) -> (Document, Position) {
        let marker = marked.find('¦').expect("cursor marker");
        let mut document = Document::new(7, marked.replacen('¦', "", 1)).unwrap();
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, Position::default()).unwrap();
        let position = snapshot.lines.position(snapshot.text, marker).unwrap();
        (document, position)
    }

    fn apply(marked: &str, new_name: &str) -> Result<String, ResponseError> {
        let (mut document, position) = marked_document(marked);
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, position)?;
        let edits = rename(
            &snapshot,
            position,
            new_name,
            &parser,
            &Cancellation::default(),
        )?;
        document.change(
            8,
            edits
                .into_iter()
                .rev()
                .map(|edit| ContentChange {
                    range: Some(edit.range),
                    text: edit.new_text,
                })
                .collect(),
        )?;
        Ok(document
            .snapshot(&parser, Position::default())?
            .text
            .to_string())
    }

    #[test]
    fn renames_precise_utf16_labels_without_touching_display_text_or_resources() {
        let marked = "😀 [shown][ol¦d] ![alt][OLD]\r\n\r\n[old]: /url \"Title\"\r\n";
        let (mut document, position) = marked_document(marked);
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, position).unwrap();
        let prepared = prepare(&snapshot, position).unwrap();
        assert_eq!(prepared.placeholder, "old");
        assert_eq!(
            prepared.range,
            Range {
                start: Position {
                    line: 0,
                    character: 11
                },
                end: Position {
                    line: 0,
                    character: 14
                },
            }
        );
        assert_eq!(
            apply(marked, "新的😀").unwrap(),
            "😀 [shown][新的😀] ![alt][新的😀]\r\n\r\n[新的😀]: /url \"Title\"\r\n"
        );
    }

    #[test]
    fn expands_implicit_references_while_preserving_their_rendered_labels() {
        assert_eq!(
            apply("[ol¦d] [old][] ![old] ![old][]\n\n[old]: /target\n", "new").unwrap(),
            "[old][new] [old][new] ![old][new] ![old][new]\n\n[new]: /target\n"
        );
        let (mut document, position) = marked_document("[old][¦]\n\n[old]: /target");
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, position).unwrap();
        let prepared = prepare(&snapshot, position).unwrap();
        assert_eq!(prepared.range.start, prepared.range.end);
        assert_eq!(prepared.placeholder, "old");
    }

    #[test]
    fn keeps_implicit_forms_for_case_only_renames_and_returns_noop_for_same_name() {
        let marked = "[ol¦d] [old][] [Shown][OLD] ![old]\n\n[old]: /target";
        assert_eq!(
            apply(marked, "Old").unwrap(),
            "[old] [old][] [Shown][Old] ![old]\n\n[Old]: /target"
        );
        let (mut document, position) = marked_document(marked);
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, position).unwrap();
        assert!(rename(
            &snapshot,
            position,
            "old",
            &parser,
            &Cancellation::default()
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn keeps_footnotes_separate_and_preserves_duplicate_definition_order() {
        assert_eq!(
            apply(
                "[^ol¦d] [text][old]\n\n[^old]: note **body**\n\n[old]: /url\n[new]: /link-new\n",
                "new"
            )
            .unwrap(),
            "[^new] [text][old]\n\n[^new]: note **body**\n\n[old]: /url\n[new]: /link-new\n"
        );
        assert_eq!(
            apply("[ol¦d]\n\n[OLD]: /first\n[old]: /second\n", "next").unwrap(),
            "[old][next]\n\n[next]: /first\n[next]: /second\n"
        );
    }

    #[test]
    fn locates_escaped_and_multiline_labels_inside_containers() {
        assert_eq!(
            apply("[show][A\\]¦B]\n\n[A\\]B]: /path", "C\\[D").unwrap(),
            "[show][C\\[D]\n\n[C\\[D]: /path"
        );
        let marked = "> [old\n> label]: /url\n>\n> [shown][old\n> la¦bel]\n";
        assert_eq!(
            apply(marked, "new").unwrap(),
            "> [new]: /url\n>\n> [shown][new]\n"
        );
        assert_eq!(
            apply("  [^ol¦d]: body\n\n[^old]", "new").unwrap(),
            "  [^new]: body\n\n[^new]"
        );
    }

    #[test]
    fn refuses_non_label_positions_and_inactive_definitions() {
        for marked in [
            "[sh¦own][old]\n\n[old]: /url",
            "[old]: /u¦rl",
            "[^old]: bo¦dy\n\n[^old]",
            "[old]: /first\n[ol¦d]: /second",
            "`[ol¦d]`\n\n[old]: /url",
            "[miss¦ing][]",
        ] {
            let (mut document, position) = marked_document(marked);
            let parser = YozoraParser::default();
            let snapshot = document.snapshot(&parser, position).unwrap();
            assert!(prepare(&snapshot, position).is_none(), "{marked}");
            assert_eq!(
                rename(
                    &snapshot,
                    position,
                    "new",
                    &parser,
                    &Cancellation::default()
                )
                .unwrap_err()
                .code,
                -32803
            );
        }
    }

    #[test]
    fn rejects_invalid_names_collisions_and_changes_to_unrelated_markdown() {
        let marked = "[ol¦d]\n\n[old]: /url\n[other]: /other";
        for name in [
            "",
            " ",
            " leading",
            "trailing ",
            "a\nb",
            "a\rb",
            "a\0b",
            "[nested]",
            "broken]",
            "^note",
        ] {
            assert_eq!(apply(marked, name).unwrap_err().code, -32602, "{name:?}");
        }
        assert_eq!(apply(marked, "OTHER").unwrap_err().code, -32803);
        assert_eq!(
            apply("[ol¦d] [new]\n\n[old]: /url", "new")
                .unwrap_err()
                .code,
            -32803
        );
        assert_eq!(
            apply("| col |\n| --- |\n| [ol¦d] |\n\n[old]: /url", "a|b")
                .unwrap_err()
                .code,
            -32803
        );
        assert_eq!(
            apply("[ol¦d] `[new]`\n\n[old]: /url", "new").unwrap(),
            "[old][new] `[new]`\n\n[new]: /url"
        );
    }

    #[test]
    fn rejects_edits_that_would_exceed_the_document_limit() {
        let marked = format!("[ol¦d]: /url\n\n{}", "[old]\n\n".repeat(4_500));
        let error = apply(&marked, &"😀".repeat(999)).unwrap_err();
        assert!(error.message.contains("16 MiB"));
    }

    #[test]
    fn validates_deep_documents_without_recursive_ast_serialization() {
        let prefix = "> ".repeat(256);
        let marked = format!("[old]: /url\n\n{prefix}[ol¦d]");
        assert_eq!(
            apply(&marked, "new").unwrap(),
            format!("[new]: /url\n\n{prefix}[old][new]")
        );
    }
}
