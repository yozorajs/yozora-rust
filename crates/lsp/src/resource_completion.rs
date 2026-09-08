use yozora_ast::{Node, Root};
use yozora_ast_util::{collect_definitions, collect_footnote_definitions};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::analysis::nodes;
use crate::cancellation::Cancellation;
use crate::completion::{association_bytes, parse_options, MAX_PROBE_BYTES};
use crate::document::{check_size, Snapshot};
use crate::files::{self, PathCandidate};
use crate::links;
use crate::protocol::{CompletionItem, CompletionList, Position, Range, ResponseError, TextEdit};

const BEGIN: &str = "yozora_completion_6f79_begin_";
const CURSOR: &str = "yozora_completion_6f79_cursor_";
const END: &str = "yozora_completion_6f79_end_";

#[derive(Clone, Copy, Eq, PartialEq)]
enum ResourceKind {
    Link,
    Image,
    Definition,
}

fn resource(node: &Node) -> Option<(ResourceKind, &str)> {
    match node {
        Node::Link(node) => Some((ResourceKind::Link, &node.url)),
        Node::Image(node) => Some((ResourceKind::Image, &node.url)),
        Node::Definition(node) => Some((ResourceKind::Definition, &node.url)),
        _ => None,
    }
}

pub enum Target<'a> {
    Path(&'a str),
    Anchor { resource: &'a str, prefix: &'a str },
}

pub struct Candidate {
    label: String,
    uri: String,
    kind: u32,
    detail: &'static str,
}

/// An owned probe lets the server release the source borrow before reading a
/// target buffer. All returned edits replace only the destination's contents.
pub struct Context {
    destination: String,
    cursor: usize,
    filter_prefix: String,
    range: Range,
    before: String,
    after: String,
    resource_kind: ResourceKind,
    resource_start: Position,
    options: ParseOptions,
    remaining: usize,
    document_bytes: usize,
    replaced_bytes: usize,
}

impl Context {
    pub fn new(
        snapshot: &Snapshot<'_>,
        position: Position,
        parser: &YozoraParser,
        cancellation: &Cancellation,
    ) -> Result<Option<Self>, ResponseError> {
        cancellation.check()?;
        let (line, cursor) = snapshot.line(position)?;
        let mut introducers: Vec<_> = line[..cursor]
            .rmatch_indices("](")
            .take(16)
            .map(|(offset, _)| (offset, false))
            .chain(
                line[..cursor]
                    .rmatch_indices("]:")
                    .take(16)
                    .map(|(offset, _)| (offset, true)),
            )
            .collect();
        if introducers.is_empty() {
            return Ok(None);
        }
        introducers.sort_unstable_by_key(|&(offset, _)| std::cmp::Reverse(offset));
        let in_table = nodes(&snapshot.root.children).any(|node| {
            matches!(node, Node::Table(_))
                && node
                    .position()
                    .is_some_and(|range| Range::from(range).contains(position))
        });
        let introducers: Vec<_> = introducers
            .into_iter()
            .take(16)
            .filter_map(|(introducer, definition)| {
                destination_span(line, introducer + 2, cursor, in_table)
                    .map(|span| (introducer, definition, span))
            })
            .collect();
        if introducers.is_empty() {
            return Ok(None);
        }
        let options = parse_options(
            &collect_definitions(snapshot.root),
            &collect_footnote_definitions(snapshot.root),
        );
        let associations = association_bytes(&options);
        let mut remaining = MAX_PROBE_BYTES;
        let line_start = snapshot.lines.byte_offset(
            snapshot.text,
            Position {
                line: position.line,
                character: 0,
            },
        )?;
        for (introducer, definition, span) in introducers {
            cancellation.check()?;
            let opening = Position {
                line: position.line,
                character: line[..introducer].encode_utf16().count() as u32,
            };
            let Some(block) = snapshot
                .root
                .children
                .iter()
                .filter_map(Node::position)
                .map(Range::from)
                .find(|range| range.contains(opening))
            else {
                continue;
            };
            let block_start = snapshot.lines.byte_offset(
                snapshot.text,
                Position {
                    line: block.start.line,
                    character: 0,
                },
            )?;
            let block_end = snapshot
                .lines
                .byte_offset(snapshot.text, block.end)?
                .max(line_start + line.len());
            let block_text = &snapshot.text[block_start..block_end];
            if block_text.len() + associations + BEGIN.len() + CURSOR.len() + END.len() > remaining
            {
                return Ok(None);
            }
            if [BEGIN, CURSOR, END]
                .iter()
                .any(|marker| block_text.contains(marker))
            {
                continue;
            }
            let before = &snapshot.text[block_start..line_start + span.start];
            let after = &snapshot.text[line_start + span.end..block_end];
            let marked = format!(
                "{BEGIN}{}{CURSOR}{}{END}",
                &line[span.start..cursor],
                &line[cursor..span.end]
            );
            let closing = if span.angle && !span.closed_angle {
                if definition {
                    ">"
                } else {
                    ">)"
                }
            } else if definition {
                ""
            } else {
                ")"
            };
            let mut after_variants = vec![after.to_string()];
            if span.syntax_end == line.len()
                && (cursor == span.end || span.closed_angle)
                && !closing.is_empty()
            {
                let preserved = span.syntax_end - span.end;
                after_variants.push(format!(
                    "{}{closing}{}",
                    &after[..preserved],
                    &after[preserved..]
                ));
            }
            for after in after_variants {
                cancellation.check()?;
                let cost = before.len() + marked.len() + after.len() + associations;
                if cost > remaining {
                    return Ok(None);
                }
                remaining -= cost;
                let parsed =
                    parser.parse(format!("{before}{marked}{after}"), Some(options.clone()));
                cancellation.check()?;
                for node in nodes(&parsed.children) {
                    let Some((kind, url)) = resource(node) else {
                        continue;
                    };
                    if (kind == ResourceKind::Definition) != definition {
                        continue;
                    }
                    let Some((prefix, suffix)) = url
                        .strip_prefix(BEGIN)
                        .and_then(|url| url.strip_suffix(END))
                        .and_then(|url| url.split_once(CURSOR))
                    else {
                        continue;
                    };
                    let Some(range) = node.position().map(Range::from) else {
                        continue;
                    };
                    if range.start
                        >= (Position {
                            line: opening.line - block.start.line,
                            character: opening.character,
                        })
                    {
                        continue;
                    }
                    return Ok(Some(Self {
                        destination: format!("{prefix}{suffix}"),
                        cursor: prefix.len(),
                        filter_prefix: line[span.start..cursor].to_string(),
                        range: Range {
                            start: Position {
                                line: position.line,
                                character: line[..span.start].encode_utf16().count() as u32,
                            },
                            end: Position {
                                line: position.line,
                                character: line[..span.end].encode_utf16().count() as u32,
                            },
                        },
                        before: before.to_string(),
                        after,
                        resource_kind: kind,
                        resource_start: range.start,
                        options,
                        remaining,
                        document_bytes: snapshot.text.len(),
                        replaced_bytes: span.end - span.start,
                    }));
                }
            }
        }
        Ok(None)
    }

    pub fn target(&self) -> Option<Target<'_>> {
        let resource_end = self
            .destination
            .find(['?', '#'])
            .unwrap_or(self.destination.len());
        let resource = &self.destination[..resource_end];
        if self.cursor < files::completion_path_start(resource)? {
            return None;
        }
        let prefix = &self.destination[..self.cursor];
        if let Some((resource, prefix)) = prefix.split_once('#') {
            Some(Target::Anchor { resource, prefix })
        } else if prefix.contains('?') {
            None
        } else {
            // Splitting `%2F` around the cursor would hide the separator from
            // both halves and let a filename edit consume the remaining path.
            if files::path_separators(resource)
                .any(|separator| separator.start < self.cursor && self.cursor < separator.end)
            {
                return None;
            }
            Some(Target::Path(prefix))
        }
    }

    pub fn paths(&self, entries: Vec<PathCandidate>) -> Vec<Candidate> {
        let Some(Target::Path(prefix)) = self.target() else {
            return Vec::new();
        };
        let suffix = &self.destination[self.cursor..];
        let parent = files::path_separators(prefix)
            .next_back()
            .map_or("", |separator| &prefix[..separator.end]);
        let suffix_start = suffix.find(['?', '#']).unwrap_or(suffix.len());
        let (path_suffix, query_fragment) = suffix.split_at(suffix_start);
        let path_suffix = files::path_separators(path_suffix)
            .next()
            .map_or("", |separator| &path_suffix[separator.start..]);
        entries
            .into_iter()
            .filter_map(|entry| {
                if !path_suffix.is_empty() && !entry.directory {
                    return None;
                }
                let slash = if entry.directory && path_suffix.is_empty() {
                    "/"
                } else {
                    ""
                };
                Some(Candidate {
                    label: format!("{}{}", entry.name, if entry.directory { "/" } else { "" }),
                    uri: format!(
                        "{parent}{}{slash}{path_suffix}{query_fragment}",
                        files::encode_component(&entry.name)
                    ),
                    kind: if entry.directory { 19 } else { 17 },
                    detail: if entry.directory { "Directory" } else { "File" },
                })
            })
            .collect()
    }

    pub fn anchors(&self, root: &Root, heading_prefix: &str) -> Vec<Candidate> {
        let Some(Target::Anchor { resource, prefix }) = self.target() else {
            return Vec::new();
        };
        let Some(prefix) = files::component_prefix(prefix) else {
            return Vec::new();
        };
        links::headings(root, heading_prefix)
            .filter(|(identifier, _)| {
                !identifier.is_empty() && files::encode_component(identifier).starts_with(&prefix)
            })
            .take(200)
            .map(|(identifier, _)| Candidate {
                uri: format!("{resource}#{}", files::encode_component(&identifier)),
                label: identifier,
                kind: 18,
                detail: "Heading",
            })
            .collect()
    }

    pub fn complete(
        mut self,
        candidates: Vec<Candidate>,
        parser: &YozoraParser,
        cancellation: &Cancellation,
    ) -> Result<CompletionList, ResponseError> {
        cancellation.check()?;
        let mut items = Vec::new();
        let associations = association_bytes(&self.options);
        for candidate in candidates.into_iter().take(200) {
            cancellation.check()?;
            let Some(uri) = files::encode_uri(&candidate.uri) else {
                continue;
            };
            // Entity and delimiter escaping preserves the intended URI through
            // Markdown parsing, including ampersands followed by entity names.
            let new_text = uri
                .replace('&', "&amp;")
                .replace('(', "\\(")
                .replace(')', "\\)");
            let cost = self.before.len()
                + self.after.len()
                + new_text.len()
                + associations
                + candidate.label.len()
                + self.filter_prefix.len()
                + new_text.len()
                + candidate.detail.len();
            if cost > self.remaining {
                break;
            }
            self.remaining -= cost;
            if check_size(self.document_bytes - self.replaced_bytes + new_text.len()).is_err() {
                continue;
            }
            let parsed = parser.parse(
                format!("{}{new_text}{}", self.before, self.after),
                Some(self.options.clone()),
            );
            cancellation.check()?;
            if !nodes(&parsed.children).any(|node| {
                resource(node)
                    .is_some_and(|(kind, value)| kind == self.resource_kind && value == uri)
                    && node
                        .position()
                        .is_some_and(|range| Position::from(range.start) == self.resource_start)
            }) {
                continue;
            }
            items.push(CompletionItem {
                label: candidate.label,
                kind: candidate.kind,
                detail: candidate.detail.to_string(),
                filter_text: self.filter_prefix.clone(),
                text_edit: TextEdit {
                    range: self.range,
                    new_text,
                },
            });
        }
        Ok(CompletionList {
            is_incomplete: true,
            items,
        })
    }
}

struct DestinationSpan {
    start: usize,
    end: usize,
    syntax_end: usize,
    angle: bool,
    closed_angle: bool,
}

fn destination_span(
    line: &str,
    mut start: usize,
    cursor: usize,
    in_table: bool,
) -> Option<DestinationSpan> {
    while matches!(line.as_bytes().get(start), Some(b' ' | b'\t')) {
        start += 1;
    }
    let angle = line.as_bytes().get(start) == Some(&b'<');
    if angle {
        start += 1;
    }
    if cursor < start {
        return None;
    }
    let mut end = line.len();
    let mut depth = 0usize;
    let mut escaped = false;
    let mut closed_angle = false;
    for (offset, current) in line[start..].char_indices() {
        let offset = start + offset;
        if current.is_ascii_control() || (!angle && current.is_ascii_whitespace()) {
            end = offset;
            break;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match current {
            '\\' => escaped = true,
            '|' if in_table => {
                end = offset;
                break;
            }
            '>' if angle => {
                end = offset;
                closed_angle = true;
                break;
            }
            '<' => {
                end = offset;
                break;
            }
            '(' if !angle => depth += 1,
            ')' if !angle => {
                if depth == 0 {
                    end = offset;
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    if cursor > end {
        return None;
    }
    let prefix = &line[start..cursor];
    if prefix
        .bytes()
        .rev()
        .take_while(|&byte| byte == b'\\')
        .count()
        % 2
        != 0
    {
        return None;
    }
    // Inserting a probe marker inside an entity would change the URI being
    // completed, for example `docs&so|l;guide.md` would lose its `/` separator.
    if prefix.rsplit_once('&').is_some_and(|(_, left)| {
        line[cursor..end].split_once(';').is_some_and(|(right, _)| {
            (!left.is_empty() || !right.is_empty())
                && left
                    .chars()
                    .chain(right.chars())
                    .all(|character| character.is_ascii_alphanumeric() || character == '#')
        })
    }) {
        return None;
    }
    Some(DestinationSpan {
        start,
        end,
        syntax_end: end + usize::from(closed_angle),
        angle,
        closed_angle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::protocol::ContentChange;

    fn request_context(marked: &str) -> (Document, Option<Context>) {
        let marker = marked.find('¦').unwrap();
        let before = &marked[..marker];
        let position = Position {
            line: before.bytes().filter(|&byte| byte == b'\n').count() as u32,
            character: before.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
        };
        let mut document = Document::new(1, marked.replacen('¦', "", 1)).unwrap();
        let parser = YozoraParser::default();
        let context = Context::new(
            &document.snapshot(&parser, position).unwrap(),
            position,
            &parser,
            &Cancellation::default(),
        )
        .unwrap();
        (document, context)
    }

    fn entry(name: &str, directory: bool) -> PathCandidate {
        PathCandidate {
            name: name.to_string(),
            directory,
        }
    }

    fn accept(document: &mut Document, item: CompletionItem) -> String {
        document
            .change(
                2,
                vec![ContentChange {
                    range: Some(item.text_edit.range),
                    text: item.text_edit.new_text,
                }],
            )
            .unwrap();
        document
            .snapshot(&YozoraParser::default(), Position::default())
            .unwrap()
            .text
            .to_string()
    }

    #[test]
    fn replaces_destinations_while_preserving_titles_queries_fragments_and_display() {
        for (marked, expected) in [
            (
                "[go](docs/gu¦ide.md#intro \"Title\")",
                "[go](docs/guide2.md#intro \"Title\")",
            ),
            (
                "[ref]: <docs/gu¦ide.md?view=raw#intro> \"Title\"",
                "[ref]: <docs/guide2.md?view=raw#intro> \"Title\"",
            ),
            ("[ref]: docs/gu¦ide.md", "[ref]: docs/guide2.md"),
            ("[go](docs&sol;gu¦ide.md)", "[go](docs/guide2.md)"),
            ("[go](docs&#x2f;gu¦ide.md)", "[go](docs/guide2.md)"),
            (
                "[go](docs%2Fgu¦ide.md#intro)",
                "[go](docs%2Fguide2.md#intro)",
            ),
            ("[go](docs%2F¦guide.md)", "[go](docs%2Fguide2.md)"),
            (
                "[go](FILE://localhost/docs/gu¦ide.md)",
                "[go](FILE://localhost/docs/guide2.md)",
            ),
            ("[go](file:/docs/gu¦ide.md)", "[go](file:/docs/guide2.md)"),
            (
                "[![alt](docs/gu¦ide.md)](outer.md)",
                "[![alt](docs/guide2.md)](outer.md)",
            ),
            (
                ":::note [go](docs/gu¦ide.md)\nbody\n:::",
                ":::note [go](docs/guide2.md)\nbody\n:::",
            ),
            (
                "> [go](docs/gu¦ide.md)\n> body",
                "> [go](docs/guide2.md)\n> body",
            ),
            ("[go\nhere](docs/gu¦ide.md)", "[go\nhere](docs/guide2.md)"),
        ] {
            let (mut document, context) = request_context(marked);
            let context = context.unwrap_or_else(|| panic!("missing context: {marked}"));
            let candidates = context.paths(vec![entry("guide2.md", false)]);
            let mut result = context
                .complete(
                    candidates,
                    &YozoraParser::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(result.items.len(), 1, "{marked}");
            assert_eq!(accept(&mut document, result.items.remove(0)), expected);
        }
    }

    #[test]
    fn preserves_remaining_path_components_when_completing_a_directory() {
        for (marked, expected) in [
            (
                "[go](docs/pa¦rt/file.md?raw#intro)",
                "[go](docs/parts/file.md?raw#intro)",
            ),
            (
                "[go](docs/part¦%2Ffile.md?raw#intro)",
                "[go](docs/parts%2Ffile.md?raw#intro)",
            ),
            (
                "[go](docs/pa¦rt%2ffile.md?raw#intro)",
                "[go](docs/parts%2ffile.md?raw#intro)",
            ),
            (
                "[go](%2Fdocs/pa¦rt/file.md?raw#intro)",
                "[go](%2Fdocs/parts/file.md?raw#intro)",
            ),
        ] {
            let (mut document, context) = request_context(marked);
            let context = context.unwrap();
            let candidates = context.paths(vec![entry("parts", true), entry("page.md", false)]);
            let mut result = context
                .complete(
                    candidates,
                    &YozoraParser::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(result.items.len(), 1, "{marked}");
            assert_eq!(result.items[0].kind, 19);
            assert_eq!(result.items[0].label, "parts/");
            assert_eq!(accept(&mut document, result.items.remove(0)), expected);
        }
    }

    #[test]
    fn encodes_unicode_and_table_pipes_and_preserves_markdown_uri_escapes() {
        let (mut document, context) = request_context("😀 [go](中文/文¦档.md)");
        let context = context.unwrap();
        let candidates = context.paths(vec![entry("文件.md", false)]);
        let mut result = context
            .complete(
                candidates,
                &YozoraParser::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].text_edit.range.start.character, 8);
        assert_eq!(result.items[0].text_edit.range.end.character, 16);
        assert_eq!(
            accept(&mut document, result.items.remove(0)),
            "😀 [go](%E4%B8%AD%E6%96%87/%E6%96%87%E4%BB%B6.md)"
        );

        let (mut document, context) =
            request_context("| A | B |\n| - | - |\n| [go](pi¦pe.md) | neighbor |");
        let context = context.unwrap();
        let candidates = context.paths(vec![entry("pipe|name.md", false)]);
        let mut result = context
            .complete(
                candidates,
                &YozoraParser::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(result.items.len(), 1);
        assert!(accept(&mut document, result.items.remove(0))
            .contains("[go](pipe%7Cname.md) | neighbor"));

        let (mut document, context) =
            request_context("[go](docs\\(v1\\)/gu¦ide.md?copy=&amp;copy; \"Title\")");
        let context = context.unwrap();
        let candidates = context.paths(vec![entry("guide2.md", false)]);
        let mut result = context
            .complete(
                candidates,
                &YozoraParser::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(result.items.len(), 1);
        accept(&mut document, result.items.remove(0));
        let root = document.ast(&YozoraParser::default()).unwrap();
        assert!(nodes(&root.children)
            .any(|node| resource(node)
                .is_some_and(|(_, url)| url == "docs(v1)/guide2.md?copy=&copy;")));
    }

    #[test]
    fn supports_unfinished_destinations_without_inserting_syntax_delimiters() {
        for (marked, expected) in [
            ("[go](gu¦", "[go](guide.md"),
            ("![alt](<gu¦", "![alt](<guide.md"),
            ("[ref]: <gu¦", "[ref]: <guide.md"),
            ("[go](<gu¦>", "[go](<guide.md>"),
            ("[ref]: gu¦", "[ref]: guide.md"),
        ] {
            let (mut document, context) = request_context(marked);
            let context = context.unwrap_or_else(|| panic!("missing context: {marked}"));
            let candidates = context.paths(vec![entry("guide.md", false)]);
            let mut result = context
                .complete(
                    candidates,
                    &YozoraParser::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(result.items.len(), 1, "{marked}");
            assert_eq!(accept(&mut document, result.items.remove(0)), expected);
        }
        let (_, context) = request_context("[go](do¦");
        let context = context.unwrap();
        let candidates = context.paths(vec![entry("docs", true)]);
        assert_eq!(
            context
                .complete(
                    candidates,
                    &YozoraParser::default(),
                    &Cancellation::default()
                )
                .unwrap()
                .items[0]
                .text_edit
                .new_text,
            "docs/"
        );
    }

    #[test]
    fn anchors_share_navigation_ids_and_accept_partial_encoded_prefixes() {
        let headings = "# Intro\n# Intro\n# 中文😀\n#\n\n";
        for (suffix, expected) in [
            ("[go](#¦)", vec!["intro", "intro-2", "中文😀"]),
            ("[go](#in¦tro-extra)", vec!["intro", "intro-2"]),
            ("[go](#%E4¦)", vec!["中文😀"]),
            ("[go](#Intro¦)", vec![]),
            ("[go](guide.md?view=raw#in¦)", vec!["intro", "intro-2"]),
        ] {
            let (mut document, context) = request_context(&format!("{headings}{suffix}"));
            let context = context.unwrap();
            let candidates = context.anchors(document.ast(&YozoraParser::default()).unwrap(), "");
            let result = context
                .complete(
                    candidates,
                    &YozoraParser::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(
                result
                    .items
                    .iter()
                    .map(|item| item.label.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "{suffix}"
            );
        }
        let (mut document, context) = request_context(&format!("{headings}[go](#h-in¦)"));
        let context = context.unwrap();
        let candidates = context.anchors(document.ast(&YozoraParser::default()).unwrap(), "h-");
        let result = context
            .complete(
                candidates,
                &YozoraParser::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(result.items[1].text_edit.new_text, "#h-intro-2");
    }

    #[test]
    fn rejects_non_destination_contexts_and_does_not_reinterpret_following_text() {
        for marked in [
            "text gu¦",
            "`[x](gu¦)`",
            "```md\n[x](gu¦)\n```",
            "$[x](gu¦)$",
            "<script>\n[x](gu¦)\n</script>",
            "<https://example.test/gu¦>",
            "[gu¦](file.md)",
            "[x](file.md \"gu¦\")",
            "[x][gu¦]",
            "[^x]: gu¦",
            "text [x]: gu¦",
            "\\[x](gu¦)",
            "`before\n[x](gu¦) after`",
            "[x](gu¦ rest)",
            "[x](gu¦ide",
            "[x](gu¦ide next)",
            "[go](docs&so¦l;guide.md)",
            "[go](docs&#x2¦f;guide.md)",
            "[go](file\\¦)name.md)",
        ] {
            assert!(request_context(marked).1.is_none(), "{marked}");
        }
        for marked in [
            "[x](file.md?query¦)",
            "[go](¦/docs/file.md)",
            "[go](¦%2Fdocs/file.md)",
            "[go](file:¦///docs/file.md)",
            "[go](file://local¦host/docs/file.md)",
            "[go](file://host/docs/fi¦le.md)",
            "[go](%2F%2Fhost/docs/fi¦le.md)",
        ] {
            let (_, context) = request_context(marked);
            assert!(context.unwrap().target().is_none(), "{marked}");
        }
    }

    #[test]
    fn bounds_context_probes_and_repeated_completion_output() {
        let marked = format!("{} [go](gu¦)", "x".repeat(MAX_PROBE_BYTES));
        assert!(request_context(&marked).1.is_none());
        let marked = format!("[go]({}¦)", "a".repeat(300_000));
        let (_, context) = request_context(&marked);
        let context = context.unwrap();
        let candidates = context.paths((0..200).map(|_| entry("guide.md", false)).collect());
        let result = context
            .complete(
                candidates,
                &YozoraParser::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert!(result.items.len() < 4);
        assert!(serde_json::to_vec(&result).unwrap().len() < MAX_PROBE_BYTES);
    }
}
