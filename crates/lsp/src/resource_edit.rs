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

#[derive(Clone)]
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
    pub range: Range,
    pub replacement: Replacement,
    pub name: String,
}

struct LiteralResource {
    url: Box<str>,
    node_range: ByteRange<usize>,
    span: Option<(ByteRange<usize>, Range)>,
}

/// Immutable destinations from one parsed source. A literal URI can be replaced
/// without reparsing when its delimiters are unambiguous and neither URI can
/// introduce Markdown syntax. Everything else uses the full structural check.
pub struct Summary {
    resources: Box<[LiteralResource]>,
    bytes: usize,
}

impl Summary {
    pub fn new(
        snapshot: &Snapshot<'_>,
        cancellation: &Cancellation,
    ) -> Result<Option<Self>, ResponseError> {
        let mut resources = Vec::new();
        let mut bytes = std::mem::size_of::<Self>();
        for node in nodes(&snapshot.root.children) {
            cancellation.check()?;
            let Some(url) = destination(node) else {
                continue;
            };
            if resources.len() == 20_000 {
                return Err(limit(
                    "rename exceeds the resource count limit; use narrower workspace roots",
                ));
            }
            bytes += std::mem::size_of::<LiteralResource>() + url.len();
            if bytes > 8 * 1024 * 1024 {
                return Ok(None);
            }
            let position = node
                .position()
                .ok_or_else(|| failed("a resource has no source position"))?;
            let start = snapshot
                .lines
                .byte_offset(snapshot.text, position.start.into())?;
            let end = snapshot
                .lines
                .byte_offset(snapshot.text, position.end.into())?;
            let span = literal_span(&snapshot.text[start..end], node).map(|span| {
                let span = start + span.start..start + span.end;
                let range = Range {
                    start: snapshot
                        .lines
                        .position(snapshot.text, span.start)
                        .expect("a literal destination starts on a character boundary"),
                    end: snapshot
                        .lines
                        .position(snapshot.text, span.end)
                        .expect("a literal destination ends on a character boundary"),
                };
                (span, range)
            });
            resources.push(LiteralResource {
                url: url.into(),
                node_range: start..end,
                span,
            });
        }
        Ok(Some(Self {
            resources: resources.into_boxed_slice(),
            bytes,
        }))
    }

    pub fn owned_bytes(&self) -> usize {
        self.bytes
    }

    pub fn rewrite(
        &self,
        text: &str,
        heading: Option<&HeadingChange>,
        mut transform: impl FnMut(&str) -> Result<Option<String>, ResponseError>,
        cancellation: &Cancellation,
        budget: &mut Budget,
    ) -> Result<Option<Vec<TextEdit>>, ResponseError> {
        // A fallback must see the original budget, including resources visited
        // before the first nonliteral destination was encountered.
        let mut candidate = budget.clone();
        let mut replacements = Vec::new();
        // The heading plan has already validated this replacement in its source
        // context. Literal URL edits outside it cannot alter that structure.
        if let Some(heading) = heading {
            let replacement = Replacement {
                range: heading.replacement.range.clone(),
                text: heading.replacement.text.clone(),
            };
            candidate.edit(&replacement)?;
            replacements.push((replacement, heading.range));
        }
        for resource in &self.resources {
            cancellation.check()?;
            candidate.resources = candidate.resources.checked_sub(1).ok_or_else(|| {
                limit("rename exceeds the resource count limit; use narrower workspace roots")
            })?;
            if heading.is_some_and(|heading| {
                heading.replacement.range.start <= resource.node_range.start
                    && resource.node_range.end <= heading.replacement.range.end
            }) {
                continue;
            }
            let Some(url) = transform(&resource.url)? else {
                continue;
            };
            if url == resource.url.as_ref() {
                continue;
            }
            let url = files::encode_uri(&url)
                .ok_or_else(|| failed("the renamed destination cannot form a URI"))?;
            let Some((span, range)) = resource.span.as_ref().filter(|_| literal_uri(&url)) else {
                return Ok(None);
            };
            let replacement = Replacement {
                range: span.clone(),
                text: url,
            };
            candidate.edit(&replacement)?;
            replacements.push((replacement, *range));
        }
        replacements.sort_unstable_by_key(|(replacement, _)| replacement.range.start);
        let mut previous = 0;
        let mut length = text.len();
        let mut growth = 0;
        for (replacement, _) in &replacements {
            if replacement.range.start < previous || replacement.range.end > text.len() {
                return Err(failed("rename edits overlap or have invalid source ranges"));
            }
            previous = replacement.range.end;
            length = length - replacement.range.len() + replacement.text.len();
            growth += replacement
                .text
                .len()
                .saturating_sub(replacement.range.len());
        }
        check_size(text.len() + growth)?;
        if !replacements.is_empty() {
            // Retain the same work limits as full validation, even though this
            // proof avoids constructing and parsing the complete preview.
            candidate.parse(length)?;
        }
        *budget = candidate;
        Ok(Some(
            replacements
                .into_iter()
                .map(|(replacement, range)| TextEdit {
                    range,
                    new_text: replacement.text,
                })
                .collect(),
        ))
    }
}

fn literal_uri(url: &str) -> bool {
    !url.is_empty()
        && url.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'-' | b'.' | b'_' | b'~' | b'/' | b'%' | b'#' | b'?' | b'=' | b':' | b'+'
                )
        })
}

fn literal_span(text: &str, node: &Node) -> Option<ByteRange<usize>> {
    let url = destination(node)?;
    if !literal_uri(url) {
        return None;
    }
    let definition = matches!(node, Node::Definition(_));
    let prefix = if matches!(node, Node::Image(_)) {
        "!["
    } else {
        "["
    };
    if !text.starts_with(prefix) {
        return None;
    }
    let mut delimiters = text.match_indices(if definition { "]:" } else { "](" });
    let (offset, _) = delimiters.next()?;
    if delimiters.next().is_some() {
        return None;
    }
    let (span, end) = destination_span(text, offset + 2)?;
    if text[span.clone()] != *url || (!definition && !link_tail(&text[end..])) {
        return None;
    }
    Some(span)
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

/// Validate a heading once while preparing its anchor plan. A plain, top-level
/// ATX heading occupies one line, so its unchanged block delimiters isolate the
/// edit from the rest of the document. More involved headings use full parsing.
pub fn heading_preview(
    snapshot: &Snapshot<'_>,
    change: &HeadingChange,
    parser: &YozoraParser,
    cancellation: &Cancellation,
    budget: &mut Budget,
) -> Result<Root, ResponseError> {
    let headings: Vec<_> = snapshot
        .root
        .children
        .iter()
        .filter(|node| matches!(node, Node::Heading(_)))
        .collect();
    let plain = headings.iter().all(|node| {
        matches!(node, Node::Heading(heading)
        if heading.children.iter().all(|node| matches!(node, Node::Text(_))))
    });
    if plain {
        if let Some((index, node)) = headings.iter().enumerate().find(|(_, node)| {
            node.position()
                .is_some_and(|position| Position::from(position.start) == change.node_start)
        }) {
            let position = node.position().unwrap();
            let start = snapshot
                .lines
                .byte_offset(snapshot.text, position.start.into())?;
            let end = snapshot
                .lines
                .byte_offset(snapshot.text, position.end.into())?;
            let raw = &snapshot.text[start..end];
            if raw.starts_with('#')
                && !raw.trim_end_matches(['\r', '\n']).contains(['\r', '\n'])
                && start <= change.replacement.range.start
                && change.replacement.range.end <= end
                && !change.replacement.text.contains(['\r', '\n'])
            {
                let replacement = Replacement {
                    range: change.replacement.range.start - start
                        ..change.replacement.range.end - start,
                    text: change.replacement.text.clone(),
                };
                check_size(
                    snapshot.text.len()
                        + replacement
                            .text
                            .len()
                            .saturating_sub(replacement.range.len()),
                )?;
                let text = apply(raw, &[replacement])?;
                let mut candidate_budget = budget.clone();
                let mut parsed = parse(&text, parser, cancellation, &mut candidate_budget)?;
                candidate_budget.parse(snapshot.text.len() - raw.len())?;
                let mut before = Root::default();
                before.children = vec![(**node).clone()];
                if preserves(
                    &before,
                    &parsed,
                    Some(change),
                    &BTreeMap::new(),
                    cancellation,
                )? {
                    let mut children: Vec<_> = headings.into_iter().cloned().collect();
                    children[index] = parsed.children.pop().expect("validated ATX heading");
                    *budget = candidate_budget;
                    let mut root = Root::default();
                    root.children = children;
                    return Ok(root);
                }
            }
        }
    }
    let text = apply(snapshot.text, std::slice::from_ref(&change.replacement))?;
    let root = parse(&text, parser, cancellation, budget)?;
    if !preserves(
        snapshot.root,
        &root,
        Some(change),
        &BTreeMap::new(),
        cancellation,
    )? {
        return Err(failed(
            "heading rename would change other Markdown content or structure",
        ));
    }
    Ok(root)
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

    #[test]
    fn isolated_heading_validation_skips_body_parsing_and_matches_full_preview_ids() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        use yozora_core_parser::DefaultParserProps;
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        let parser = YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    counted.fetch_add(1, Ordering::Relaxed);
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        });
        let cancellation = Cancellation::default();
        for ending in ["\n", "\r\n"] {
            let source =
                format!("# Old{ending}{ending}[body](count){ending}{ending}## Old{ending}");
            let mut document = Document::new(1, source).unwrap();
            let snapshot = document.snapshot(&parser, Position::default()).unwrap();
            let calls_before = calls.load(Ordering::Relaxed);
            let change = HeadingChange {
                node_start: Position::default(),
                range: Range {
                    start: Position {
                        line: 0,
                        character: 2,
                    },
                    end: Position {
                        line: 0,
                        character: 5,
                    },
                },
                replacement: Replacement {
                    range: 2..5,
                    text: "New".into(),
                },
                name: "New".into(),
            };
            let mut quick_budget = Budget::default();
            let quick = heading_preview(
                &snapshot,
                &change,
                &parser,
                &cancellation,
                &mut quick_budget,
            )
            .unwrap();
            assert_eq!(
                calls.load(Ordering::Relaxed),
                calls_before,
                "body must not be reparsed for a literal ATX heading"
            );
            let text = apply(snapshot.text, std::slice::from_ref(&change.replacement)).unwrap();
            let mut full_budget = Budget::default();
            let full = parse(&text, &parser, &cancellation, &mut full_budget).unwrap();
            assert!(preserves(
                snapshot.root,
                &full,
                Some(&change),
                &BTreeMap::new(),
                &cancellation
            )
            .unwrap());
            assert_eq!(
                yozora_ast_util::calc_heading_identifiers(&quick, "h-"),
                yozora_ast_util::calc_heading_identifiers(&full, "h-")
            );
            assert_eq!(quick_budget.parse_bytes, full_budget.parse_bytes);
        }
    }

    #[test]
    fn literal_summaries_match_complete_parsing_for_varied_destinations_and_contexts() {
        let parser = YozoraParser::default();
        let cancellation = Cancellation::default();
        for source in [
            "😀 [shown](old.md#intro \"title\") [self](#intro)",
            "![alt](<old.md#intro> 'title')",
            "[shown][r]\n\n[r]: old.md#intro \"title\"\n[R]: other.md#intro",
            "[r]:\n  <old.md#intro>\n  \"title\"\n\n[r]",
            "> [go](\n> old.md#intro\n> \"title\")",
            ":::note [go](old.md#intro)\nbody\n:::",
            "| A | B |\n| - | - |\n| [go](old.md#intro) | other |",
            "[real](old.md#intro) `[code](old.md#intro)`",
            "## [heading](old.md#intro)\r\n\r\n[go](old.md#intro)",
        ] {
            let mut document = Document::new(1, source.to_string()).unwrap();
            let snapshot = document.snapshot(&parser, Position::default()).unwrap();
            let summary = Summary::new(&snapshot, &cancellation).unwrap().unwrap();
            for replacement in ["new", "new-2", "%E4%B8%AD%E6%96%87", "new_3.4~5"] {
                let transform = |url: &str| Ok(Some(url.replace("intro", replacement)));
                let mut quick_budget = Budget::default();
                let mut full_budget = Budget::default();
                let quick = summary
                    .rewrite(source, None, transform, &cancellation, &mut quick_budget)
                    .unwrap()
                    .unwrap_or_else(|| panic!("literal fallback: {source}"));
                let full = rewrite(
                    &snapshot,
                    None,
                    transform,
                    &parser,
                    &cancellation,
                    &mut full_budget,
                )
                .unwrap_or_else(|error| panic!("{source}: {}", error.message));
                assert_eq!(
                    serde_json::to_value(quick).unwrap(),
                    serde_json::to_value(full).unwrap(),
                    "{source}"
                );
                assert_eq!(
                    (
                        quick_budget.parse_bytes,
                        quick_budget.resources,
                        quick_budget.edits,
                        quick_budget.edit_bytes
                    ),
                    (
                        full_budget.parse_bytes,
                        full_budget.resources,
                        full_budget.edits,
                        full_budget.edit_bytes
                    ),
                    "{source}",
                );
            }
        }
    }

    #[test]
    fn nonliteral_and_ambiguous_destinations_fall_back_without_spending_the_budget() {
        let parser = YozoraParser::default();
        let cancellation = Cancellation::default();
        for source in [
            "[go](old\\(x\\).md)",
            "[go](old&#46;md)",
            "[go](<old 中文.md>)",
            "[outer ![inner](old.md)](old.md)",
            "![`x](wrong)`](old.md \"title ](wrong)\")",
            "[first](old.md) <file:///tmp/old.md>",
        ] {
            let mut document = Document::new(1, source.to_string()).unwrap();
            let snapshot = document.snapshot(&parser, Position::default()).unwrap();
            let summary = Summary::new(&snapshot, &cancellation).unwrap().unwrap();
            let mut budget = Budget::default();
            assert!(
                summary
                    .rewrite(
                        source,
                        None,
                        |url| Ok(Some(url.replace("old", "new"))),
                        &cancellation,
                        &mut budget
                    )
                    .unwrap()
                    .is_none(),
                "{source}"
            );
            let expected = Budget::default();
            assert_eq!(
                (budget.resources, budget.edits, budget.edit_bytes),
                (expected.resources, expected.edits, expected.edit_bytes)
            );
        }
    }

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
