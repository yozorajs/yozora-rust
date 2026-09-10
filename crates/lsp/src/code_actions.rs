use std::collections::BTreeMap;

use serde_json::{json, Value};
use yozora_ast::{Definition, Node, Root};
use yozora_parser::YozoraParser;

use crate::analysis::{node_signature, nodes, plain_text};
use crate::cancellation::Cancellation;
use crate::document::Snapshot;
use crate::protocol::{Range, ResponseError, TextEdit};
use crate::resource_edit::{self, Budget, Replacement};

pub const ORGANIZE: &str = "source.organizeLinkDefinitions";
pub const EXTRACT: &str = "refactor.extract.linkDefinition";

pub struct Action {
    pub title: &'static str,
    pub kind: &'static str,
    pub edits: Vec<TextEdit>,
}

/// Actions are complete edits against the source version. Reparse the preview
/// before returning it, including unrelated reference bindings and block syntax.
pub fn actions(
    snapshot: &Snapshot<'_>,
    range: Range,
    only: Option<&[String]>,
    parser: &YozoraParser,
    cancellation: &Cancellation,
) -> Result<Vec<Action>, ResponseError> {
    cancellation.check()?;
    let enabled = |kind: &str| {
        only.is_none_or(|kinds| {
            kinds.iter().any(|parent| {
                parent.is_empty()
                    || kind == parent
                    || kind
                        .strip_prefix(parent.as_str())
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
        })
    };
    let mut result = Vec::new();
    let mut budget = Budget::default();
    if enabled(ORGANIZE) {
        if let Some(replacements) = organize(snapshot, cancellation)? {
            if let Some(edits) = validate(
                snapshot,
                replacements,
                None,
                parser,
                cancellation,
                &mut budget,
            )? {
                result.push(Action {
                    title: "Organize link definitions",
                    kind: ORGANIZE,
                    edits,
                });
            }
        }
    }
    if enabled(EXTRACT) {
        let target = nodes(&snapshot.root.children)
            .filter(|node| {
                destination(node).is_some()
                    && node.position().is_some_and(|position| {
                        let node = Range::from(position);
                        if range.start == range.end {
                            node.start <= range.start && range.start <= node.end
                        } else {
                            node.start < range.end && range.start < node.end
                        }
                    })
            })
            .last();
        if let Some(target) = target {
            // Avoid both existing definitions and ordinary shortcut spellings.
            // Full preview validation also catches whitespace-normalized labels.
            let definitions = definitions(snapshot.root);
            let lowercase = snapshot.text.to_ascii_lowercase();
            let mut attempts = 0;
            for suffix in 1..=128 {
                cancellation.check()?;
                let label = if suffix == 1 {
                    "link".into()
                } else {
                    format!("link{suffix}")
                };
                if definitions.contains_key(label.as_str())
                    || lowercase.contains(&format!("[{label}]"))
                {
                    continue;
                }
                let Some(replacements) = extract(snapshot, target, &label, cancellation)? else {
                    break;
                };
                if let Some(edits) = validate(
                    snapshot,
                    replacements,
                    Some(&label),
                    parser,
                    cancellation,
                    &mut budget,
                )? {
                    result.push(Action {
                        title: "Extract to link definition",
                        kind: EXTRACT,
                        edits,
                    });
                    break;
                }
                attempts += 1;
                if attempts == 8 {
                    break;
                }
            }
        }
    }
    Ok(result)
}

fn organize(
    snapshot: &Snapshot<'_>,
    cancellation: &Cancellation,
) -> Result<Option<Vec<Replacement>>, ResponseError> {
    let mut entries = Vec::new();
    let mut replacements = Vec::new();
    // Nested definitions retain their container and indentation. Moving them
    // could remove a list item or alter a footnote's contents.
    for node in &snapshot.root.children {
        cancellation.check()?;
        let Node::Definition(definition) = node else {
            continue;
        };
        let Some(position) = node.position() else {
            return Ok(None);
        };
        let range = Range::from(position);
        let start = snapshot.lines.byte_offset(snapshot.text, range.start)?;
        let end = snapshot.lines.byte_offset(snapshot.text, range.end)?;
        let (first_line, first_offset) = snapshot.line(range.start)?;
        if !first_line[..first_offset].trim().is_empty() {
            return Ok(None);
        }
        let mut line_end = end;
        // Parser spans may already include the definition's final line ending.
        if range.end.character != 0 || start == end {
            let (last_line, last_offset) = snapshot.line(range.end)?;
            if !last_line[last_offset..].trim().is_empty() {
                return Ok(None);
            }
            line_end += last_line.len() - last_offset;
            if snapshot.text[line_end..].starts_with("\r\n") {
                line_end += 2;
            } else if snapshot.text[line_end..].starts_with(['\r', '\n']) {
                line_end += 1;
            }
        }
        entries.push((
            definition.identifier.as_str(),
            snapshot.text[start..end].trim_end_matches(['\r', '\n']),
        ));
        replacements.push(Replacement {
            range: start - first_offset..line_end,
            text: String::new(),
        });
    }
    if entries.is_empty() {
        return Ok(None);
    }
    entries.sort_by_key(|&(identifier, _)| identifier);
    let remainder = resource_edit::apply(snapshot.text, &replacements)?;
    let newline = newline(snapshot.text);
    let mut block = separator(&remainder, newline).to_string();
    block.push_str(
        &entries
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join(newline),
    );
    if snapshot.text.ends_with(['\r', '\n']) {
        block.push_str(newline);
    }
    replacements.push(Replacement {
        range: snapshot.text.len()..snapshot.text.len(),
        text: block,
    });
    Ok(Some(replacements))
}

fn extract(
    snapshot: &Snapshot<'_>,
    target: &Node,
    label: &str,
    cancellation: &Cancellation,
) -> Result<Option<Vec<Replacement>>, ResponseError> {
    let Some(expected) = destination(target) else {
        return Ok(None);
    };
    let mut replacements = Vec::new();
    let mut definition = None;
    for node in nodes(&snapshot.root.children) {
        cancellation.check()?;
        if destination(node) != Some(expected) {
            continue;
        }
        let Some(position) = node.position() else {
            return Ok(None);
        };
        let start = snapshot
            .lines
            .byte_offset(snapshot.text, position.start.into())?;
        let end = snapshot
            .lines
            .byte_offset(snapshot.text, position.end.into())?;
        let raw = &snapshot.text[start..end];
        if let Some(closing) = closing_display(raw) {
            if std::ptr::eq(node, target) {
                definition = Some(raw[closing + 2..raw.len() - 1].trim().to_string());
            }
            replacements.push(Replacement {
                range: start + closing + 1..end,
                text: format!("[{label}]"),
            });
        } else if let Node::Link(link) = node {
            let display = plain_text(&link.children);
            let mut escaped = String::new();
            for character in display.chars() {
                if character.is_ascii_punctuation() {
                    escaped.push('\\');
                }
                escaped.push(character);
            }
            if std::ptr::eq(node, target) {
                // Autolinks have no title. Encoding makes the angle destination
                // independent of Markdown delimiter and entity spellings.
                let Some(url) = crate::files::encode_uri(&link.url) else {
                    return Ok(None);
                };
                definition = Some(format!(
                    "<{}>",
                    url.replace('&', "&amp;")
                        .replace('<', "%3C")
                        .replace('>', "%3E")
                ));
            }
            replacements.push(Replacement {
                range: start..end,
                text: format!("[{escaped}][{label}]"),
            });
        } else {
            return Ok(None);
        }
    }
    let Some(definition) = definition else {
        return Ok(None);
    };
    let newline = newline(snapshot.text);
    let ending = if snapshot.text.ends_with(['\r', '\n']) {
        newline
    } else {
        ""
    };
    replacements.push(Replacement {
        range: snapshot.text.len()..snapshot.text.len(),
        text: format!(
            "{}[{label}]: {definition}{ending}",
            separator(snapshot.text, newline)
        ),
    });
    replacements.sort_by_key(|replacement| (replacement.range.start, replacement.range.end));
    if replacements
        .windows(2)
        .any(|pair| pair[0].range.end > pair[1].range.start)
    {
        return Ok(None);
    }
    Ok(Some(replacements))
}

fn destination(node: &Node) -> Option<(&str, Option<&str>)> {
    match node {
        Node::Link(link) => Some((&link.url, link.title.as_deref())),
        Node::Image(image) => Some((&image.url, image.title.as_deref())),
        _ => None,
    }
}

fn closing_display(raw: &str) -> Option<usize> {
    let start = if raw.starts_with("![") {
        2
    } else if raw.starts_with('[') {
        1
    } else {
        return None;
    };
    if !raw.ends_with(')') {
        return None;
    }
    let bytes = raw.as_bytes();
    let mut depth = 1;
    let mut cursor = start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => {
                cursor += 2;
                continue;
            }
            b'`' => {
                let width = bytes[cursor..]
                    .iter()
                    .take_while(|&&byte| byte == b'`')
                    .count();
                let marker = &raw[cursor..cursor + width];
                if let Some((offset, _)) =
                    raw[cursor + width..]
                        .match_indices(marker)
                        .find(|(offset, _)| {
                            let found = cursor + width + offset;
                            bytes.get(found.wrapping_sub(1)) != Some(&b'`')
                                && bytes.get(found + width) != Some(&b'`')
                        })
                {
                    cursor += width + offset + width;
                    continue;
                }
                cursor += width;
                continue;
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return (bytes.get(cursor + 1) == Some(&b'(')).then_some(cursor);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else if text.contains('\r') && !text.contains('\n') {
        "\r"
    } else {
        "\n"
    }
}

fn separator<'a>(text: &str, newline: &'a str) -> &'a str {
    if text.is_empty() || text.ends_with(&format!("{newline}{newline}")) {
        ""
    } else if text.ends_with(newline) {
        newline
    } else {
        match newline {
            "\r\n" => "\r\n\r\n",
            "\r" => "\r\r",
            _ => "\n\n",
        }
    }
}

fn validate(
    snapshot: &Snapshot<'_>,
    replacements: Vec<Replacement>,
    added: Option<&str>,
    parser: &YozoraParser,
    cancellation: &Cancellation,
    budget: &mut Budget,
) -> Result<Option<Vec<TextEdit>>, ResponseError> {
    if replacements.len() > 10_000
        || replacements
            .iter()
            .map(|replacement| 256 + 6 * replacement.text.len())
            .sum::<usize>()
            > 4 * 1024 * 1024
    {
        return Err(ResponseError::new(
            -32000,
            "code action exceeds its edit budget",
        ));
    }
    cancellation.check()?;
    let edited = resource_edit::apply(snapshot.text, &replacements)?;
    if edited == snapshot.text {
        return Ok(None);
    }
    let root = resource_edit::parse(&edited, parser, cancellation, budget)?;
    if !preserves(snapshot.root, &root, added, cancellation)? {
        return Ok(None);
    }
    replacements
        .into_iter()
        .map(|replacement| {
            Ok(TextEdit {
                range: Range {
                    start: snapshot
                        .lines
                        .position(snapshot.text, replacement.range.start)?,
                    end: snapshot
                        .lines
                        .position(snapshot.text, replacement.range.end)?,
                },
                new_text: replacement.text,
            })
        })
        .collect::<Result<Vec<_>, ResponseError>>()
        .map(Some)
}

fn definitions(root: &Root) -> BTreeMap<&str, Vec<&Definition>> {
    let mut definitions: BTreeMap<&str, Vec<&Definition>> = BTreeMap::new();
    for node in nodes(&root.children) {
        if let Node::Definition(definition) = node {
            definitions
                .entry(&definition.identifier)
                .or_default()
                .push(definition);
        }
    }
    definitions
}

fn preserves(
    before: &Root,
    after: &Root,
    added: Option<&str>,
    cancellation: &Cancellation,
) -> Result<bool, ResponseError> {
    let left_definitions = definitions(before);
    let mut right_definitions = definitions(after);
    if let Some(added) = added {
        if left_definitions.contains_key(added)
            || right_definitions
                .remove(added)
                .is_none_or(|definitions| definitions.len() != 1)
        {
            return Ok(false);
        }
    }
    if left_definitions.len() != right_definitions.len() {
        return Ok(false);
    }
    for (identifier, left) in &left_definitions {
        cancellation.check()?;
        let Some(right) = right_definitions.get(identifier) else {
            return Ok(false);
        };
        if left.len() != right.len()
            || left.iter().zip(right).any(|(left, right)| {
                left.label != right.label || left.url != right.url || left.title != right.title
            })
        {
            return Ok(false);
        }
    }
    let right_definitions = definitions(after);
    let visible = |node: &&Node| !matches!(node, Node::Definition(_));
    if before.children.iter().filter(visible).count()
        != after.children.iter().filter(visible).count()
    {
        return Ok(false);
    }
    let mut left = nodes(&before.children).filter(visible);
    let mut right = nodes(&after.children).filter(visible);
    loop {
        cancellation.check()?;
        match (left.next(), right.next()) {
            (None, None) => return Ok(true),
            (Some(left), Some(right)) => {
                match (
                    signature(left, &left_definitions),
                    signature(right, &right_definitions),
                ) {
                    (Some(left), Some(right)) if left == right => {}
                    _ => return Ok(false),
                }
            }
            _ => return Ok(false),
        }
    }
}

fn signature(node: &Node, definitions: &BTreeMap<&str, Vec<&Definition>>) -> Option<Value> {
    let mut value = match node {
        Node::Link(link) => json!({ "type": "link", "url": link.url, "title": link.title }),
        Node::LinkReference(link) => {
            let definition = definitions.get(link.identifier.as_str())?.first()?;
            json!({ "type": "link", "url": definition.url, "title": definition.title })
        }
        Node::Image(image) => {
            json!({ "type": "image", "url": image.url, "title": image.title, "alt": image.alt })
        }
        Node::ImageReference(image) => {
            let definition = definitions.get(image.identifier.as_str())?.first()?;
            json!({ "type": "image", "url": definition.url, "title": definition.title, "alt": image.alt })
        }
        _ => node_signature(node)?,
    };
    if let Some(children) = node.children() {
        value.as_object_mut()?.insert(
            "childrenCount".into(),
            json!(children
                .iter()
                .filter(|node| !matches!(node, Node::Definition(_)))
                .count()),
        );
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::protocol::{ContentChange, Position};

    fn apply(text: &str, kind: &str, position: Position) -> Option<String> {
        let parser = YozoraParser::default();
        let mut document = Document::new(1, text.into()).unwrap();
        let snapshot = document.snapshot(&parser, position).unwrap();
        let actions = actions(
            &snapshot,
            Range {
                start: position,
                end: position,
            },
            Some(&[kind.into()]),
            &parser,
            &Cancellation::default(),
        )
        .unwrap();
        assert!(actions.len() <= 1);
        let mut edits = actions.into_iter().next()?.edits;
        edits.reverse();
        document
            .change(
                2,
                edits
                    .into_iter()
                    .map(|edit| ContentChange {
                        range: Some(edit.range),
                        text: edit.new_text,
                    })
                    .collect(),
            )
            .unwrap();
        Some(document.text().unwrap().to_string())
    }

    #[test]
    fn organizes_definitions_stably_without_removing_unused_or_changing_duplicate_bindings() {
        let source = "[z]: /first \"title\"\r\n\r\n[Z] and [a]\r\n\r\n[a]: /a\r\n[z]: /second\r\n[unused]: /u\r\n";
        let result = apply(source, ORGANIZE, Position::default()).unwrap();
        assert!(
            result
                .ends_with("[a]: /a\r\n[unused]: /u\r\n[z]: /first \"title\"\r\n[z]: /second\r\n"),
            "{result:?}"
        );
        assert!(apply(&result, ORGANIZE, Position::default()).is_none());
    }

    #[test]
    fn extraction_preserves_display_titles_images_and_other_reference_bindings() {
        let source = "[link] and [ LINK2 ]\n\n[中文 **bold**](https://example.org/a \"title\") ![alt](https://example.org/a \"title\") [other](https://example.org/a \"different\")\n";
        let result = apply(
            source,
            EXTRACT,
            Position {
                line: 2,
                character: 5,
            },
        )
        .unwrap();
        assert!(
            result.contains(
                "[中文 **bold**][link3] ![alt][link3] [other](https://example.org/a \"different\")"
            ),
            "{result:?}"
        );
        assert!(result.ends_with("[link3]: https://example.org/a \"title\"\n"));
        assert!(result.starts_with("[link] and [ LINK2 ]"));
    }

    #[test]
    fn handles_autolinks_nested_images_and_code_delimiters() {
        for (source, position) in [
            (
                "<https://example.org/a?q=1&b=2>",
                Position {
                    line: 0,
                    character: 8,
                },
            ),
            (
                "[![alt](u)](u)",
                Position {
                    line: 0,
                    character: 4,
                },
            ),
            (
                "[`a]b`](u)",
                Position {
                    line: 0,
                    character: 3,
                },
            ),
            (
                "| link |\n| --- |\n| [a\\|b](u) |",
                Position {
                    line: 2,
                    character: 4,
                },
            ),
        ] {
            let result = apply(source, EXTRACT, position)
                .unwrap_or_else(|| panic!("no extraction: {source}"));
            assert!(result.contains("[link]:"), "{result}");
        }
    }

    #[test]
    fn respects_kind_hierarchy_and_unsafe_or_inapplicable_contexts() {
        assert!(apply(
            "`[a](u)`",
            EXTRACT,
            Position {
                line: 0,
                character: 3
            }
        )
        .is_none());
        assert!(apply(
            "[a][ref]\n\n[ref]: u",
            EXTRACT,
            Position {
                line: 0,
                character: 1
            }
        )
        .is_none());
        assert!(apply(
            "[a](u)\n\n```",
            EXTRACT,
            Position {
                line: 0,
                character: 1
            }
        )
        .is_none());
        assert!(apply(
            "[a](u)",
            "refactor",
            Position {
                line: 0,
                character: 1
            }
        )
        .is_some());
        assert!(apply(
            "[a](u)",
            "quickfix",
            Position {
                line: 0,
                character: 1
            }
        )
        .is_none());
        let source = "> [z]: /nested\n\n[z]: /outer\n[a]: /a\n\n[z]\n";
        assert!(apply(source, ORGANIZE, Position::default()).is_some());
        let source = "[z]: /outer\n\n> [z]: /nested\n\n[z]\n";
        assert!(apply(source, ORGANIZE, Position::default()).is_none());
    }
}
