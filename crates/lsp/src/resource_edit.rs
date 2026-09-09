use std::collections::BTreeMap;
use std::ops::Range as ByteRange;

use serde_json::json;
use yozora_ast::{Node, Root};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::analysis::{node_signature, nodes, plain_text};
use crate::cancellation::Cancellation;
use crate::document::{check_size, Snapshot};
use crate::files;
use crate::protocol::{Position, Range, ResponseError, TextEdit};

pub struct Budget {
    pub parse_bytes: usize,
    pub resources: usize,
    pub edits: usize,
    pub edit_bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            parse_bytes: 128 * 1024 * 1024,
            resources: 20_000,
            edits: 10_000,
            edit_bytes: 4 * 1024 * 1024,
        }
    }
}

impl Budget {
    pub fn document(&mut self, uri: &str) -> Result<(), ResponseError> {
        self.edit_bytes = self
            .edit_bytes
            .checked_sub(128 + 6 * uri.len())
            .ok_or_else(|| limit("rename exceeds the edit byte limit"))?;
        Ok(())
    }

    pub fn parse(&mut self, bytes: usize) -> Result<(), ResponseError> {
        self.parse_bytes = self.parse_bytes.checked_sub(bytes).ok_or_else(|| {
            limit("rename analysis exceeds its parse budget; use narrower workspace roots")
        })?;
        Ok(())
    }

    fn edit(&mut self, replacement: &Replacement) -> Result<(), ResponseError> {
        self.edits = self
            .edits
            .checked_sub(1)
            .ok_or_else(|| limit("rename exceeds the edit count limit"))?;
        self.edit_bytes = self
            .edit_bytes
            .checked_sub(256 + 6 * replacement.text.len())
            .ok_or_else(|| limit("rename exceeds the edit byte limit"))?;
        Ok(())
    }
}

pub struct Replacement {
    pub range: ByteRange<usize>,
    pub text: String,
}

pub struct HeadingChange {
    pub node_start: Position,
    pub replacement: Replacement,
    pub name: String,
}

pub fn parse(
    text: &str,
    parser: &YozoraParser,
    cancellation: &Cancellation,
    budget: &mut Budget,
) -> Result<Root, ResponseError> {
    cancellation.check()?;
    budget.parse(text.len())?;
    let root = parser.parse(
        text,
        Some(ParseOptions {
            should_reserve_position: Some(true),
            ..ParseOptions::default()
        }),
    );
    cancellation.check()?;
    Ok(root)
}

pub fn apply(text: &str, replacements: &[Replacement]) -> Result<String, ResponseError> {
    let mut length = text.len();
    let mut growth = 0;
    let mut previous = 0;
    for replacement in replacements {
        if replacement.range.start < previous
            || replacement.range.start > replacement.range.end
            || text.get(replacement.range.clone()).is_none()
        {
            return Err(failed("rename edits overlap or have invalid source ranges"));
        }
        previous = replacement.range.end;
        length = length - replacement.range.len() + replacement.text.len();
        growth += replacement
            .text
            .len()
            .saturating_sub(replacement.range.len());
    }
    // Clients may apply disjoint edits in either order before sending didChange.
    check_size(text.len() + growth)?;
    let mut result = String::with_capacity(length);
    let mut cursor = 0;
    for replacement in replacements {
        result.push_str(&text[cursor..replacement.range.start]);
        result.push_str(&replacement.text);
        cursor = replacement.range.end;
    }
    result.push_str(&text[cursor..]);
    Ok(result)
}

/// Locate only affected destinations, then validate the complete edited document.
/// The parser must preserve all payloads and bindings outside an explicitly
/// replaced heading and the URLs named by the transformation.
pub fn rewrite(
    snapshot: &Snapshot<'_>,
    heading: Option<&HeadingChange>,
    mut transform: impl FnMut(&str) -> Result<Option<String>, ResponseError>,
    parser: &YozoraParser,
    cancellation: &Cancellation,
    budget: &mut Budget,
) -> Result<Vec<TextEdit>, ResponseError> {
    let mut replacements = Vec::new();
    let mut expected_urls = BTreeMap::new();
    if let Some(heading) = heading {
        let replacement = Replacement {
            range: heading.replacement.range.clone(),
            text: heading.replacement.text.clone(),
        };
        budget.edit(&replacement)?;
        replacements.push(replacement);
    }
    for node in nodes(&snapshot.root.children) {
        cancellation.check()?;
        let Some(url) = destination(node) else {
            continue;
        };
        budget.resources = budget.resources.checked_sub(1).ok_or_else(|| {
            limit("rename exceeds the resource count limit; use narrower workspace roots")
        })?;
        let position = node
            .position()
            .ok_or_else(|| failed("a resource has no source position"))?;
        let node_start = snapshot
            .lines
            .byte_offset(snapshot.text, position.start.into())?;
        let node_end = snapshot
            .lines
            .byte_offset(snapshot.text, position.end.into())?;
        if heading.is_some_and(|heading| {
            heading.replacement.range.start <= node_start
                && node_end <= heading.replacement.range.end
        }) {
            continue;
        }
        let Some(new_url) = transform(url)? else {
            continue;
        };
        if new_url == url {
            continue;
        }
        let new_url = files::encode_uri(&new_url)
            .ok_or_else(|| failed("the renamed destination cannot form a URI"))?;
        let range = destination_range(
            snapshot,
            node,
            node_start..node_end,
            parser,
            cancellation,
            budget,
        )?;
        let replacement = Replacement {
            range,
            text: new_url
                .replace('&', "&amp;")
                .replace('(', "\\(")
                .replace(')', "\\)"),
        };
        budget.edit(&replacement)?;
        replacements.push(replacement);
        expected_urls.insert(Position::from(position.start), new_url);
    }
    if replacements.is_empty() {
        return Ok(Vec::new());
    }
    replacements.sort_by_key(|replacement| (replacement.range.start, replacement.range.end));
    let text = apply(snapshot.text, &replacements)?;
    let root = parse(&text, parser, cancellation, budget)?;
    if !preserves(snapshot.root, &root, heading, &expected_urls, cancellation)? {
        return Err(failed(
            "rename would change other Markdown content, structure, or reference bindings",
        ));
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
        .collect()
}

pub fn preserves(
    before: &Root,
    after: &Root,
    heading: Option<&HeadingChange>,
    expected_urls: &BTreeMap<Position, String>,
    cancellation: &Cancellation,
) -> Result<bool, ResponseError> {
    let mut stack = vec![(before.children.as_slice(), after.children.as_slice())];
    while let Some((left, right)) = stack.pop() {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right) {
            cancellation.check()?;
            let (Some(mut expected), Some(mut actual)) =
                (node_signature(left), node_signature(right))
            else {
                return Ok(false);
            };
            let start = left
                .position()
                .map(|position| Position::from(position.start));
            if let Some(heading) = heading.filter(|heading| {
                start == Some(heading.node_start) && matches!(left, Node::Heading(_))
            }) {
                let Node::Heading(right) = right else {
                    return Ok(false);
                };
                expected.as_object_mut().unwrap().remove("childrenCount");
                actual.as_object_mut().unwrap().remove("childrenCount");
                let name = heading
                    .name
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if expected != actual
                    || plain_text(&right.children) != name
                    || right
                        .children
                        .iter()
                        .any(|node| !matches!(node, Node::Text(_)))
                {
                    return Ok(false);
                }
                continue;
            }
            if destination(left).is_some() {
                if let Some(url) = start.and_then(|start| expected_urls.get(&start)) {
                    expected["url"] = json!(url);
                }
            }
            if expected != actual {
                return Ok(false);
            }
            if let (Some(left), Some(right)) = (left.children(), right.children()) {
                stack.push((left, right));
            }
            if let (Node::Admonition(left), Node::Admonition(right)) = (left, right) {
                stack.push((&left.title, &right.title));
            }
        }
    }
    Ok(true)
}

fn destination(node: &Node) -> Option<&str> {
    match node {
        Node::Link(node) => Some(&node.url),
        Node::Image(node) => Some(&node.url),
        Node::Definition(node) => Some(&node.url),
        _ => None,
    }
}

fn destination_range(
    snapshot: &Snapshot<'_>,
    node: &Node,
    range: ByteRange<usize>,
    parser: &YozoraParser,
    cancellation: &Cancellation,
    budget: &mut Budget,
) -> Result<ByteRange<usize>, ResponseError> {
    let text = &snapshot.text[range.clone()];
    let definition = matches!(node, Node::Definition(_));
    let mut candidates: Box<dyn Iterator<Item = usize> + '_> = if definition {
        Box::new(text.match_indices("]:").map(|(offset, _)| offset))
    } else {
        Box::new(text.rmatch_indices("](").map(|(offset, _)| offset))
    };
    for offset in candidates.by_ref().take(16) {
        cancellation.check()?;
        if text[..offset]
            .bytes()
            .rev()
            .take_while(|&byte| byte == b'\\')
            .count()
            % 2
            != 0
        {
            continue;
        }
        let Some((span, syntax_end)) = destination_span(text, offset + 2) else {
            continue;
        };
        if !definition && !link_tail(&text[syntax_end..]) {
            continue;
        }
        let raw = &text[span.clone()];
        let url = destination(node).unwrap();
        let matches = if raw == url {
            true
        } else {
            let probe = format!("[x]({})", &text[skip_prefix(text, offset + 2)..syntax_end]);
            let parsed = parse(&probe, parser, cancellation, budget)?;
            let matches = nodes(&parsed.children)
                .any(|node| matches!(node, Node::Link(link) if link.url == url));
            matches
        };
        if matches {
            return Ok(range.start + span.start..range.start + span.end);
        }
    }
    Err(failed(
        "cannot locate an editable Markdown destination without changing its display or title",
    ))
}

fn skip_prefix(text: &str, mut offset: usize) -> usize {
    let mut continuation = false;
    while let Some(&byte) = text.as_bytes().get(offset) {
        if byte.is_ascii_whitespace() {
            continuation |= matches!(byte, b'\r' | b'\n');
            offset += 1;
        } else if continuation && byte == b'>' {
            offset += 1;
        } else {
            break;
        }
    }
    offset
}

fn destination_span(text: &str, offset: usize) -> Option<(ByteRange<usize>, usize)> {
    let start = skip_prefix(text, offset);
    let angle = text.as_bytes().get(start) == Some(&b'<');
    let start = start + usize::from(angle);
    let mut depth = 0;
    let mut escaped = false;
    for (offset, character) in text.get(start..)?.char_indices() {
        let offset = start + offset;
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '>' if angle => return Some((start..offset, offset + 1)),
            '<' if angle => return None,
            '\r' | '\n' if angle => return None,
            '(' if !angle => depth += 1,
            ')' if !angle && depth == 0 => return Some((start..offset, offset)),
            ')' if !angle => depth -= 1,
            _ if !angle && character.is_ascii_whitespace() => return Some((start..offset, offset)),
            _ => {}
        }
    }
    (!angle && depth == 0).then_some((start..text.len(), text.len()))
}

fn link_tail(text: &str) -> bool {
    let text = text[skip_prefix(text, 0)..].trim_end();
    if text == ")" {
        return true;
    }
    let Some(delimiter) = text.chars().next() else {
        return false;
    };
    let closing = match delimiter {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return false,
    };
    let mut escaped = false;
    for (offset, character) in text[1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
        } else if character == closing {
            let rest = &text[offset + 2..];
            return rest[skip_prefix(rest, 0)..].trim_end() == ")";
        }
    }
    false
}

pub fn failed(message: &str) -> ResponseError {
    ResponseError::new(-32803, message)
}

fn limit(message: &str) -> ResponseError {
    ResponseError::new(-32000, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::protocol::ContentChange;

    fn edited(source: &str) -> Result<String, ResponseError> {
        let parser = YozoraParser::default();
        let mut document = Document::new(1, source.to_string()).unwrap();
        let edits = rewrite(
            &document.snapshot(&parser, Position::default()).unwrap(),
            None,
            |url| Ok(url.contains("old").then(|| url.replace("old", "new"))),
            &parser,
            &Cancellation::default(),
            &mut Budget::default(),
        )?;
        document.change(
            2,
            edits
                .into_iter()
                .rev()
                .map(|edit| ContentChange {
                    range: Some(edit.range),
                    text: edit.new_text,
                })
                .collect(),
        )?;
        Ok(document.text()?.to_string())
    }

    #[test]
    fn edits_resources_without_changing_display_titles_or_bindings() {
        for (source, expected) in [
            (
                "😀 [shown](old.md#intro \"title old.md\")",
                "😀 [shown](new.md#intro \"title old.md\")",
            ),
            (
                "![alt](<old.md#intro> 'title')",
                "![alt](<new.md#intro> 'title')",
            ),
            (
                "[outer ![inner](old.md)](old.md)",
                "[outer ![inner](new.md)](new.md)",
            ),
            (
                "[old.md][r]\n\n[r]: old.md \"old.md\"\n[R]: other.md",
                "[old.md][r]\n\n[r]: new.md \"old.md\"\n[R]: other.md",
            ),
            (
                "[r]:\n  <old.md>\n  \"title\"",
                "[r]:\n  <new.md>\n  \"title\"",
            ),
            (
                "> [go](\n> old.md\n> \"title\")",
                "> [go](\n> new.md\n> \"title\")",
            ),
            (
                ":::note [go](old.md)\nbody\n:::",
                ":::note [go](new.md)\nbody\n:::",
            ),
            (
                "| A | B |\n| - | - |\n| [go](old.md) | other |",
                "| A | B |\n| - | - |\n| [go](new.md) | other |",
            ),
            ("[go](old\\(x\\).md)", "[go](new\\(x\\).md)"),
            ("[go](old&#46;md)", "[go](new.md)"),
            ("[go](<old 中文.md>)", "[go](<new%20%E4%B8%AD%E6%96%87.md>)"),
            (
                "[real](old.md) `[code](old.md)`",
                "[real](new.md) `[code](old.md)`",
            ),
            (
                "![`x](wrong)`](old.md \"title ](wrong)\")",
                "![`x](wrong)`](new.md \"title ](wrong)\")",
            ),
        ] {
            assert_eq!(
                edited(source).unwrap_or_else(|error| panic!("{source}: {}", error.message)),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_edits_that_would_change_visible_autolink_text() {
        assert_eq!(
            edited("[go](old.md) <file:///tmp/old.md>")
                .unwrap_err()
                .code,
            -32803
        );
    }

    #[test]
    fn cancellation_and_budget_exhaustion_return_no_partial_edit_list() {
        let parser = YozoraParser::default();
        let mut document = Document::new(1, "[one](old.md) [two](old.md)".to_string()).unwrap();
        let snapshot = document.snapshot(&parser, Position::default()).unwrap();
        for (cancelled, edits) in [(true, 10), (false, 1)] {
            let cancellation = Cancellation::default();
            if cancelled {
                cancellation.cancel();
            }
            let result = rewrite(
                &snapshot,
                None,
                |_| Ok(Some("new.md".into())),
                &parser,
                &cancellation,
                &mut Budget {
                    edits,
                    ..Budget::default()
                },
            );
            assert_eq!(
                result.unwrap_err().code,
                if cancelled { -32800 } else { -32000 }
            );
        }
    }
}
