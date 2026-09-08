use yozora_ast::{Association, Definition, FootnoteDefinition, Node, Root};
use yozora_ast_util::{collect_definitions, collect_footnote_definitions};
use yozora_core_parser::ParseOptions;
use yozora_core_tokenizer::resolve_label_to_identifier;
use yozora_parser::YozoraParser;

use crate::analysis::{nodes, reference_key, single_line_label, ReferenceKey};
use crate::cancellation::Cancellation;
use crate::document::{check_size, Snapshot};
use crate::protocol::{CompletionItem, CompletionList, Position, Range, ResponseError, TextEdit};

pub(super) const MAX_PROBE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Eq, PartialEq)]
enum LabelKind {
    Link,
    Footnote,
}

struct Bracket {
    offset: usize,
    position: Position,
    kind: Option<LabelKind>,
    image: bool,
    display_start: Option<Position>,
}

struct Context {
    kind: LabelKind,
    opening: Position,
    display_start: Option<Position>,
    image: bool,
    prefix: String,
    range: Range,
    has_closing: bool,
    in_table: bool,
}

pub fn complete(
    snapshot: &Snapshot<'_>,
    position: Position,
    parser: &YozoraParser,
    cancellation: &Cancellation,
) -> Result<CompletionList, ResponseError> {
    cancellation.check()?;
    let root = snapshot.root;
    let (line, cursor) = snapshot.line(position)?;
    let Some(context) = context(root, line, cursor, position.line) else {
        return Ok(CompletionList::default());
    };
    if !is_reference_context(root, &context) {
        return Ok(CompletionList::default());
    }
    let Some(probe) = CompletionProbe::new(snapshot, &context) else {
        return Ok(CompletionList::default());
    };

    let prefix = if context.in_table {
        context.prefix.replace("\\|", "|")
    } else {
        context.prefix.clone()
    };
    let prefix = resolve_label_to_identifier(&prefix);
    let definitions = collect_definitions(root);
    let footnotes = collect_footnote_definitions(root);
    let options = parse_options(&definitions, &footnotes);
    let association_bytes = association_bytes(&options);
    let candidates: Vec<_> = match context.kind {
        LabelKind::Link => definitions
            .iter()
            .map(|definition| {
                (
                    definition.identifier.as_str(),
                    definition.label.as_str(),
                    definition.url.as_str(),
                )
            })
            .collect(),
        LabelKind::Footnote => footnotes
            .iter()
            .map(|definition| {
                (
                    definition.identifier.as_str(),
                    definition.label.as_str(),
                    "Footnote",
                )
            })
            .collect(),
    };
    let mut items = Vec::new();
    let mut remaining = MAX_PROBE_BYTES;
    for (index, (identifier, label, detail)) in candidates
        .into_iter()
        .filter(|(identifier, _, _)| identifier.starts_with(&prefix))
        .take(200)
        .enumerate()
    {
        cancellation.check()?;
        // ASCII whitespace has the same label identity, including multiline labels.
        let label = single_line_label(label);
        let mut new_text = if context.in_table {
            label.replace('|', "\\|")
        } else {
            label.clone()
        };
        if !context.has_closing {
            new_text.push(']');
        }
        let cost = probe.before.len() + new_text.len() + probe.after.len() + association_bytes;
        // Always check one candidate, even when a single block exceeds the budget.
        // Incomplete lists let typing narrow the search to later candidates.
        if index > 0 && cost > remaining {
            break;
        }
        remaining = remaining.saturating_sub(cost);
        let key = match context.kind {
            LabelKind::Link => ReferenceKey::Link(identifier),
            LabelKind::Footnote => ReferenceKey::Footnote(identifier),
        };
        if check_size(snapshot.text.len() - probe.replaced_bytes + new_text.len()).is_err()
            || !probe.accepts(
                parser,
                cancellation,
                &options,
                key,
                &new_text,
                context.has_closing,
            )?
        {
            continue;
        }
        items.push(CompletionItem {
            label,
            kind: 18, // CompletionItemKind.Reference
            detail: detail.to_string(),
            // The server applies the parser's Unicode folding. Clients need only
            // retain these matches; an incomplete list is refreshed after typing.
            filter_text: context.prefix.clone(),
            text_edit: TextEdit {
                range: context.range,
                new_text,
            },
        });
    }
    Ok(CompletionList {
        is_incomplete: true,
        items,
    })
}

pub(super) fn parse_options(
    definitions: &[&Definition],
    footnotes: &[&FootnoteDefinition],
) -> ParseOptions {
    ParseOptions {
        should_reserve_position: Some(true),
        preset_definitions: Some(
            definitions
                .iter()
                .map(|definition| Association {
                    identifier: definition.identifier.clone(),
                    label: definition.label.clone(),
                })
                .collect(),
        ),
        preset_footnote_definitions: Some(
            footnotes
                .iter()
                .map(|definition| Association {
                    identifier: definition.identifier.clone(),
                    label: definition.label.clone(),
                })
                .collect(),
        ),
        ..ParseOptions::default()
    }
}

pub(super) fn association_bytes(options: &ParseOptions) -> usize {
    options
        .preset_definitions
        .iter()
        .flatten()
        .chain(options.preset_footnote_definitions.iter().flatten())
        .map(|association| association.identifier.len() + association.label.len())
        .sum()
}

struct CompletionProbe<'a> {
    before: &'a str,
    after: &'a str,
    replaced_bytes: usize,
    reference_start: Position,
    label_start: Position,
}

impl<'a> CompletionProbe<'a> {
    fn new(snapshot: &Snapshot<'a>, context: &Context) -> Option<Self> {
        // Preserve the whole top-level block, including multiline inline content,
        // table cells and container markers. Other blocks cannot pair delimiters.
        let block = snapshot
            .root
            .children
            .iter()
            .filter_map(Node::position)
            .map(Range::from)
            .find(|range| range.contains(context.opening))?;
        let block_start = snapshot
            .lines
            .byte_offset(
                snapshot.text,
                Position {
                    line: block.start.line,
                    character: 0,
                },
            )
            .ok()?;
        let block_end = snapshot.lines.byte_offset(snapshot.text, block.end).ok()?;
        let start = snapshot
            .lines
            .byte_offset(snapshot.text, context.range.start)
            .ok()?;
        let end = snapshot
            .lines
            .byte_offset(snapshot.text, context.range.end)
            .ok()?;
        let mut reference_start = context.display_start.unwrap_or(context.opening);
        reference_start.line -= block.start.line;
        reference_start.character = reference_start
            .character
            .checked_sub(u32::from(context.image))?;
        let mut label_start = context.range.start;
        label_start.line -= block.start.line;
        Some(Self {
            before: snapshot.text.get(block_start..start)?,
            after: snapshot.text.get(end..block_end)?,
            replaced_bytes: end - start,
            reference_start,
            label_start,
        })
    }

    fn accepts(
        &self,
        parser: &YozoraParser,
        cancellation: &Cancellation,
        options: &ParseOptions,
        key: ReferenceKey<'_>,
        new_text: &str,
        has_closing: bool,
    ) -> Result<bool, ResponseError> {
        let source = format!("{}{new_text}{}", self.before, self.after);
        cancellation.check()?;
        let parsed = parser.parse(&source, Some(options.clone()));
        cancellation.check()?;
        let expected = Range {
            start: self.reference_start,
            end: Position {
                line: self.label_start.line,
                character: self.label_start.character
                    + new_text.encode_utf16().count() as u32
                    + u32::from(has_closing),
            },
        };
        let resolves = nodes(&parsed.children).any(|node| {
            reference_key(node) == Some(key) && node.position().map(Range::from) == Some(expected)
        });
        Ok(resolves)
    }
}

fn context(root: &Root, line: &str, cursor: usize, line_number: u32) -> Option<Context> {
    if !line.is_char_boundary(cursor) {
        return None;
    }
    let cursor_position = Position {
        line: line_number,
        character: line[..cursor].encode_utf16().count() as u32,
    };
    let in_table = nodes(&root.children).any(|node| {
        matches!(node, Node::Table(_))
            && node.position().is_some_and(|position| {
                let range = Range::from(position);
                range.start <= cursor_position && cursor_position <= range.end
            })
    });
    let mut ignored: Vec<_> = nodes(&root.children)
        .filter(|node| {
            matches!(
                node,
                Node::Code(_)
                    | Node::InlineCode(_)
                    | Node::Math(_)
                    | Node::InlineMath(_)
                    | Node::Html(_)
                    | Node::EcmaImport(_)
            )
        })
        .filter_map(|node| node.position().map(Range::from))
        .filter(|range| range.start.line <= line_number && line_number <= range.end.line)
        .collect();
    ignored.sort_by_key(|range| range.start);
    let mut ignored_index = 0;
    let mut stack = Vec::new();
    let mut last_closed: Option<(usize, Bracket)> = None;
    let mut last_bang = None;
    let mut escaped = false;
    let mut character = 0;

    for (offset, current) in line[..cursor].char_indices() {
        let position = Position {
            line: line_number,
            character,
        };
        character += current.len_utf16() as u32;
        while ignored
            .get(ignored_index)
            .is_some_and(|range| range.end <= position)
        {
            ignored_index += 1;
        }
        if ignored
            .get(ignored_index)
            .is_some_and(|range| range.contains(position))
        {
            escaped = false;
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match current {
            '\\' => escaped = true,
            '|' if in_table => {
                stack.clear();
                last_closed = None;
                last_bang = None;
            }
            '!' => last_bang = Some(offset),
            '[' => {
                let image = offset
                    .checked_sub(1)
                    .is_some_and(|previous| last_bang == Some(previous));
                let display = last_closed
                    .as_ref()
                    .filter(|(closing, bracket)| *closing + 1 == offset && bracket.kind.is_none());
                let kind = if display.is_some() {
                    Some(LabelKind::Link)
                } else if !image && line.as_bytes().get(offset + 1) == Some(&b'^') {
                    Some(LabelKind::Footnote)
                } else {
                    None
                };
                stack.push(Bracket {
                    offset,
                    position,
                    kind,
                    image: display.map_or(image, |(_, bracket)| bracket.image),
                    display_start: display.map(|(_, bracket)| bracket.position),
                });
            }
            ']' => last_closed = stack.pop().map(|bracket| (offset, bracket)),
            _ => {}
        }
    }
    let position = Position {
        line: line_number,
        character,
    };
    if escaped || ignored.iter().any(|range| range.contains(position)) {
        return None;
    }
    let bracket = stack.pop()?;
    let kind = bracket.kind?;
    let marker_width = if kind == LabelKind::Footnote { 2 } else { 1 };
    let start = bracket.offset + marker_width;
    if start > cursor {
        return None;
    }
    let mut prefix_length = line[start..cursor].chars().count();
    if in_table {
        prefix_length -= line[start..cursor].matches("\\|").count();
    }
    if prefix_length > 999 {
        return None;
    }
    let mut closing = None;
    let mut escaped = false;
    let mut suffix_character = character;
    let mut label_length = prefix_length;
    for (offset, current) in line[cursor..].char_indices() {
        if label_length > 999 {
            break;
        }
        let position = Position {
            line: line_number,
            character: suffix_character,
        };
        suffix_character += current.len_utf16() as u32;
        while ignored
            .get(ignored_index)
            .is_some_and(|range| range.end <= position)
        {
            ignored_index += 1;
        }
        if ignored
            .get(ignored_index)
            .is_some_and(|range| range.contains(position))
        {
            break;
        }
        // The table tokenizer removes one backslash before each escaped pipe
        // before enforcing the reference label's 999-character limit.
        if !(in_table
            && current == '\\'
            && !escaped
            && line.as_bytes().get(cursor + offset + 1) == Some(&b'|'))
        {
            label_length += 1;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match current {
            '\\' => escaped = true,
            '[' => break,
            '|' if in_table => break,
            ']' => {
                closing = Some(cursor + offset);
                break;
            }
            _ => {}
        }
    }
    let end = closing.unwrap_or(cursor);
    Some(Context {
        kind,
        opening: bracket.position,
        display_start: bracket.display_start,
        image: bracket.image,
        prefix: line[start..cursor].to_string(),
        range: Range {
            start: Position {
                line: line_number,
                character: bracket.position.character + marker_width as u32,
            },
            end: Position {
                line: line_number,
                character: character + line[cursor..end].encode_utf16().count() as u32,
            },
        },
        has_closing: closing.is_some(),
        in_table,
    })
}

fn is_reference_context(root: &Root, context: &Context) -> bool {
    let completes_link = context.kind == LabelKind::Link && !context.image;
    for node in nodes(&root.children) {
        let Some(range) = node.position().map(Range::from) else {
            continue;
        };
        if let Some(display_start) = context.display_start {
            if matches!(node, Node::Link(_) | Node::LinkReference(_))
                && display_start < range.start
                && range.end <= context.opening
            {
                return false;
            }
        }
        if !range.contains(context.opening) {
            continue;
        }
        let is_own_label = context.has_closing
            && range.end
                == (Position {
                    line: context.range.end.line,
                    character: context.range.end.character + 1,
                });
        match node {
            Node::Image(_) | Node::Definition(_) => return false,
            Node::FootnoteDefinition(definition)
                if definition
                    .children
                    .first()
                    .and_then(Node::position)
                    .is_none_or(|body| context.opening < Position::from(body.start)) =>
            {
                return false
            }
            Node::ImageReference(_) if !is_own_label => return false,
            Node::LinkReference(_) if !is_own_label && completes_link => return false,
            Node::Link(link)
                if completes_link
                    || !link.children.iter().any(|child| {
                        child
                            .position()
                            .is_some_and(|range| Range::from(range).contains(context.opening))
                    }) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use yozora_parser::YozoraParser;

    use super::*;
    use crate::document::Document;
    use crate::protocol::ContentChange;

    const DEFINITIONS: &str =
        "[Guide]: /first\n[guide]: /ignored\n[^Guide]: note\n\n[Other]: /other\n\n";

    fn request(marked: &str) -> (Document, CompletionList) {
        let marker = marked.find('¦').expect("cursor marker");
        let before = &marked[..marker];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let line_start = before.rfind('\n').map_or(0, |offset| offset + 1);
        let position = Position {
            line,
            character: before[line_start..].encode_utf16().count() as u32,
        };
        let mut document = Document::new(1, marked.replacen('¦', "", 1)).unwrap();
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, position).unwrap();
        let result = complete(&snapshot, position, &parser, &Cancellation::default()).unwrap();
        (document, result)
    }

    fn labels(result: &CompletionList) -> Vec<&str> {
        result
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect()
    }

    fn accept(marked: &str, label: &str) -> (String, Vec<(String, String)>) {
        let (mut document, result) = request(marked);
        let item = result
            .items
            .into_iter()
            .find(|item| item.label == label)
            .unwrap();
        let position = Position {
            line: item.text_edit.range.start.line,
            character: 0,
        };
        document
            .change(
                2,
                vec![ContentChange {
                    range: Some(item.text_edit.range),
                    text: item.text_edit.new_text,
                }],
            )
            .unwrap();
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, position).unwrap();
        let (line, _) = snapshot.line(position).unwrap();
        let references = nodes(&snapshot.root.children)
            .filter_map(|node| {
                let identifier = match node {
                    Node::LinkReference(reference) => &reference.identifier,
                    Node::ImageReference(reference) => &reference.identifier,
                    Node::FootnoteReference(reference) => &reference.identifier,
                    _ => return None,
                };
                Some((node.node_type().to_string(), identifier.clone()))
            })
            .collect();
        (line.to_string(), references)
    }

    #[test]
    fn completes_partial_labels_and_preserves_display_text_and_suffixes() {
        for (source, expected, node_type) in [
            ("[text][gu¦", "[text][Guide]", "linkReference"),
            (
                "[text][g¦arbage] suffix",
                "[text][Guide] suffix",
                "linkReference",
            ),
            ("[text][gu¦ suffix", "[text][Guide] suffix", "linkReference"),
            (
                "![alt][Gu¦] suffix",
                "![alt][Guide] suffix",
                "imageReference",
            ),
            ("[^gu¦]", "[^Guide]", "footnoteReference"),
        ] {
            let marked = format!("{source}\n\n{DEFINITIONS}");
            let (_, result) = request(&marked);
            assert_eq!(labels(&result), ["Guide"], "{source}");
            assert!(result.is_incomplete);
            let (line, references) = accept(&marked, "Guide");
            assert_eq!(line, expected);
            assert!(references.contains(&(node_type.to_string(), "guide".to_string())));
        }
        let (_, links) = request(&format!("[text][¦]\n\n{DEFINITIONS}"));
        assert_eq!(labels(&links), ["Guide", "Other"]);
        assert_eq!(links.items[0].detail, "/first");
        let (_, notes) = request(&format!("[^¦]\n\n{DEFINITIONS}"));
        assert_eq!(labels(&notes), ["Guide"]);
        assert_eq!(notes.items[0].detail, "Footnote");
    }

    #[test]
    fn edits_utf16_ranges_on_crlf_lines() {
        let marked = format!("# Intro\r\n😀 [text][G¦]\r\n\r\n{DEFINITIONS}");
        let (_, result) = request(&marked);
        assert_eq!(
            result.items[0].text_edit.range,
            Range {
                start: Position {
                    line: 1,
                    character: 10
                },
                end: Position {
                    line: 1,
                    character: 11
                },
            }
        );
        assert_eq!(accept(&marked, "Guide").0, "😀 [text][Guide]");
    }

    #[test]
    fn uses_parser_case_folding_ascii_whitespace_and_escaped_labels() {
        for (marked, expected_label, identifier) in [
            ("[t][SPE¦]\n\n[ſpecial]: /fold", "ſpecial", "special"),
            (
                "[t][a\t  b¦]\n\n[a b]: /space\n[a\u{00a0}b]: /nbsp",
                "a b",
                "a b",
            ),
            ("[t][A¦]\n\n[A\\]B]: /bracket", "A\\]B", "a\\]b"),
            (
                "[t][mu¦]\n\n[multi\n line]: /multi",
                "multi line",
                "multi line",
            ),
        ] {
            let (_, result) = request(marked);
            assert_eq!(labels(&result), [expected_label]);
            let (_, references) = accept(marked, expected_label);
            assert!(references.contains(&("linkReference".into(), identifier.into())));
        }
    }

    #[test]
    fn suppresses_completion_outside_reference_labels() {
        for source in [
            "plain gu¦",
            "[gu¦]",
            "\\[text][gu¦",
            "![^gu¦]",
            "[text][Guide][gu¦]",
            "`[text][gu¦]`",
            "```md\n[text][gu¦]\n```",
            "```md\n[text][gu¦",
            "    [text][gu¦]",
            "$[text][gu¦]$",
            "$$\n[text][gu¦]\n$$",
            "<div>\n[text][gu¦]\n</div>",
            "<span title=\"[text][gu¦]\">",
            "[text](/path/[other][gu¦])",
            "[text](/path/[^gu¦])",
            "![literal [text][gu¦]](/image)",
            "[^gu¦]: note",
            "  [^gu¦]: note",
            "> [^gu¦]: note",
            "- [^gu¦]: note",
            "[outer [inner][gu¦]](/target)",
            "[outer [inner][gu¦]][Other]",
        ] {
            let (_, result) = request(&format!("{DEFINITIONS}{source}"));
            assert!(result.items.is_empty(), "{source}: {:?}", result.items);
        }
    }

    #[test]
    fn handles_inline_syntax_and_reference_children_in_links() {
        for (source, node_type) in [
            ("[code `x]`][gu¦]", "linkReference"),
            ("[![alt][gu¦]](/go)", "imageReference"),
            ("[before [^gu¦]][Other]", "footnoteReference"),
            ("[before [^gu¦]](/go)", "footnoteReference"),
            ("[Other][gu¦]", "linkReference"),
            (":::note [title][gu¦]\nbody\n:::", "linkReference"),
            ("\\![^gu¦]", "footnoteReference"),
            ("[^outer]: See [^gu¦]: details", "footnoteReference"),
        ] {
            let marked = format!("{source}\n\n{DEFINITIONS}");
            let (_, references) = accept(&marked, "Guide");
            assert!(
                references.contains(&(node_type.to_string(), "guide".to_string())),
                "{source}"
            );
        }
    }

    #[test]
    fn narrows_incomplete_lists_to_labels_beyond_the_initial_limit() {
        let definitions = (0..250)
            .map(|index| format!("[id{index:03}]: /{index}\n"))
            .collect::<String>();
        let (_, initial) = request(&format!("[t][¦]\n\n{definitions}"));
        assert_eq!(initial.items.len(), 200);
        assert!(initial.is_incomplete);
        let (_, narrowed) = request(&format!("[t][id24¦]\n\n{definitions}"));
        assert_eq!(narrowed.items.len(), 10);
        assert_eq!(narrowed.items[9].label, "id249");
        assert_eq!(narrowed.items[9].filter_text, "id24");
    }

    #[test]
    fn bounds_context_parsing_and_keeps_later_labels_reachable_by_typing() {
        let definitions = (0..200)
            .map(|index| format!("[id{index:03}]: /{index}\n"))
            .collect::<String>();
        let body = "text ".repeat(3_200);
        let (_, initial) = request(&format!("{body}[t][¦]\n\n{definitions}"));
        assert!(!initial.items.is_empty());
        assert!(initial.items.len() < 200);
        assert!(initial.is_incomplete);
        let marked = format!("{body}[t][id199¦]\n\n{definitions}");
        let (_, narrowed) = request(&marked);
        assert_eq!(labels(&narrowed), ["id199"]);
        assert!(accept(&marked, "id199")
            .1
            .contains(&("linkReference".into(), "id199".into())));
    }

    #[test]
    fn confines_completion_to_one_table_cell() {
        let marked = format!("| A | B |\n| --- | --- |\n| [text][gu¦ | later ] |\n\n{DEFINITIONS}");
        let (line, references) = accept(&marked, "Guide");
        assert_eq!(line, "| [text][Guide] | later ] |");
        assert!(references.contains(&("linkReference".into(), "guide".into())));
        let (_, result) = request("| A | B |\n| --- | --- |\n| [text][a | b¦] |\n\n[a | b]: /url");
        assert!(result.items.is_empty());
    }

    #[test]
    fn escapes_table_pipes_and_matches_the_unescaped_identifier() {
        for (reference, node_type) in [
            ("[text][a¦]", "linkReference"),
            ("![alt][a¦]", "imageReference"),
            ("[^a¦]", "footnoteReference"),
        ] {
            let marked =
                format!("| C |\n| --- |\n| 😀 {reference} |\n\n[a|b]: /pipe\n[^a|b]: note\n");
            let (_, result) = request(&marked);
            assert_eq!(labels(&result), ["a|b"]);
            assert_eq!(result.items[0].text_edit.new_text, "a\\|b");
            let (line, references) = accept(&marked, "a|b");
            assert_eq!(line, format!("| 😀 {} |", reference.replace("a¦", "a\\|b")));
            assert!(references.contains(&(node_type.into(), "a|b".into())));
        }
        let definitions = "[a|b]: /pipe\n[a\\|b]: /one-slash\n[a\\\\|b]: /two-slashes";
        let marked = format!("| C |\n| --- |\n| [text][a¦] |\n\n{definitions}");
        let (_, result) = request(&marked);
        assert_eq!(labels(&result), ["a|b", "a\\\\|b"]);
        assert_eq!(result.items[1].text_edit.new_text, "a\\\\\\|b");
        assert!(accept(&marked, "a\\\\|b")
            .1
            .contains(&("linkReference".into(), "a\\\\|b".into())));
        let (_, narrowed) = request(&format!(
            "| C |\n| --- |\n| [text][a\\|¦] |\n\n{definitions}"
        ));
        assert_eq!(labels(&narrowed), ["a|b"]);
        assert_eq!(narrowed.items[0].filter_text, "a\\|");
        assert!(accept(&format!("[text][a¦]\n\n{definitions}"), "a\\|b")
            .1
            .contains(&("linkReference".into(), "a\\|b".into())));
    }

    #[test]
    fn skips_labels_that_inline_tokenizers_prevent_from_becoming_references() {
        let marked =
            "[text][¦]\n\n[`code`]: /code\n[safe]: /safe\n[a`b]: /unpaired\n[\\`code\\`]: /escaped";
        let (_, result) = request(marked);
        assert_eq!(labels(&result), ["safe", "a`b", "\\`code\\`"]);
        for label in ["safe", "a`b", "\\`code\\`"] {
            assert!(accept(marked, label)
                .1
                .contains(&("linkReference".into(), label.into())));
        }
    }

    #[test]
    fn validates_labels_in_the_surrounding_block_at_the_completed_reference() {
        for source in [
            "[text][a¦] later `",
            "`before [text][a¦]",
            "[text][a¦]\nlater `",
            "`before\n[text][a¦]",
            "![alt][a¦] later `",
            "# [text][a¦] later `",
            "> [text][a¦]\n> later `",
            "- [text][a¦]\n  later `",
            "[^note]: [text][a¦]\n    later `",
            ":::note [text][a¦] later `\nbody\n:::",
            "| C |\n| --- |\n| [text][a¦] later ` |",
            "> [existing][a`b]\n>\n> [text][a¦] later `",
        ] {
            let marked = format!("{source}\n\n[a`b]: /url\n[another]: /safe");
            let (_, result) = request(&marked);
            assert_eq!(labels(&result), ["another"], "{source}");
            assert!(accept(&marked, "another")
                .1
                .iter()
                .any(|(_, identifier)| identifier == "another"));
        }
        for source in [
            "[text][a¦]",
            "`code` [text][a¦]",
            "[code `x]`][a¦]",
            "[text][a¦]\n\nlater `",
            "| A | B |\n| --- | --- |\n| [text][a¦] | later ` |",
            "[^a¦] later `",
        ] {
            let marked = format!("{source}\n\n[a`b]: /url\n[^a`b]: note");
            let (_, result) = request(&marked);
            assert_eq!(labels(&result), ["a`b"], "{source}");
            assert!(accept(&marked, "a`b")
                .1
                .iter()
                .any(|(_, identifier)| identifier == "a`b"));
        }
    }

    #[test]
    fn measures_the_table_label_limit_after_unescaping_pipes() {
        let label = format!("a{}", "|".repeat(998));
        let spelling = label.replace('|', "\\|");
        let marked = format!("| C |\n| --- |\n| [text][{spelling}¦] |\n\n[{label}]: /url");
        let (_, result) = request(&marked);
        assert_eq!(labels(&result), [label.as_str()]);
        assert_eq!(result.items[0].text_edit.new_text, spelling);
        assert!(accept(&marked, &label)
            .1
            .contains(&("linkReference".into(), label)));
    }

    #[test]
    fn preserves_opaque_and_overlong_suffixes_after_an_unclosed_label() {
        for suffix in [
            " `later ]` tail",
            " $later ]$ tail",
            " <span title=\"x]\"> tail",
        ] {
            let marked = format!("[text][gu¦{suffix}\n\n{DEFINITIONS}");
            assert_eq!(accept(&marked, "Guide").0, format!("[text][Guide]{suffix}"));
        }
        let suffix = format!("{}]", "x".repeat(1000));
        let marked = format!("[text][gu¦{suffix}\n\n{DEFINITIONS}");
        assert_eq!(accept(&marked, "Guide").0, format!("[text][Guide]{suffix}"));
    }
}
