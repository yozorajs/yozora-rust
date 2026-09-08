use std::collections::HashMap;
use std::sync::Arc;

use yozora_ast::Root;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::protocol::{ContentChange, Position, ResponseError};

pub(super) const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const LINE_CHECKPOINT_BYTES: usize = 128;

/// The server is the only writer. Text, line index and AST belong to one version.
/// An invalid newer edit suspends queries until a full replacement restores sync.
#[derive(Clone)]
pub struct Document {
    version: i32,
    text: Arc<String>,
    lines: Arc<LineIndex>,
    synchronized: bool,
    ast: Option<Arc<Root>>,
}

pub struct Snapshot<'a> {
    pub root: &'a Root,
    pub text: &'a str,
    pub lines: &'a LineIndex,
    pub version: i32,
}

impl Snapshot<'_> {
    /// The source line and UTF-8 cursor offset, excluding the line ending.
    pub fn line(&self, position: Position) -> Result<(&str, usize), ResponseError> {
        let cursor = self.lines.byte_offset(self.text, position)?;
        let start = self.lines.starts[position.line as usize];
        let end = self
            .lines
            .starts
            .get(position.line as usize + 1)
            .copied()
            .unwrap_or(self.text.len());
        Ok((
            self.text[start..end].trim_end_matches(['\r', '\n']),
            cursor - start,
        ))
    }
}

impl Document {
    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn new(version: i32, text: String) -> Result<Self, ResponseError> {
        check_size(text.len())?;
        Ok(Self {
            version,
            lines: Arc::new(LineIndex::new(&text)),
            text: Arc::new(text),
            synchronized: true,
            ast: None,
        })
    }

    pub fn change(
        &mut self,
        version: i32,
        changes: Vec<ContentChange>,
    ) -> Result<(), ResponseError> {
        self.check_version(version)?;

        let result = self.apply_changes(changes);
        self.version = version;
        self.ast = None;
        match result {
            Ok((text, lines)) => {
                self.text = Arc::new(text);
                self.lines = Arc::new(lines);
                self.synchronized = true;
                Ok(())
            }
            Err(error) => {
                self.synchronized = false;
                Err(error)
            }
        }
    }

    /// A malformed batch with a known newer version also makes the text unknown.
    /// Keep the last text for recovery, but suspend queries and discard its AST.
    pub fn invalidate(&mut self, version: i32) -> Result<(), ResponseError> {
        self.check_version(version)?;
        self.version = version;
        self.ast = None;
        self.synchronized = false;
        Ok(())
    }

    fn check_version(&self, version: i32) -> Result<(), ResponseError> {
        if version <= self.version {
            Err(ResponseError::invalid_params(
                "document version must increase",
            ))
        } else {
            Ok(())
        }
    }

    fn apply_changes(
        &self,
        changes: Vec<ContentChange>,
    ) -> Result<(String, LineIndex), ResponseError> {
        if !self.synchronized && changes.first().is_none_or(|change| change.range.is_some()) {
            return Err(ResponseError::invalid_params(
                "a full document replacement is required to restore synchronization",
            ));
        }
        if let Some(text) = apply_descending_changes(&self.text, &self.lines, &changes)? {
            let lines = LineIndex::new(&text);
            return Ok((text, lines));
        }
        self.apply_sequential_changes(changes)
    }

    fn apply_sequential_changes(
        &self,
        changes: Vec<ContentChange>,
    ) -> Result<(String, LineIndex), ResponseError> {
        let mut text = self.text.as_ref().clone();
        let mut lines = self.lines.as_ref().clone();
        for change in changes {
            if let Some(range) = change.range {
                if range.start > range.end {
                    return Err(ResponseError::invalid_params("reversed edit range"));
                }
                let start = lines.byte_offset(&text, range.start)?;
                let end = lines.byte_offset(&text, range.end)?;
                check_size(text.len() - (end - start) + change.text.len())?;
                text.replace_range(start..end, &change.text);
            } else {
                check_size(change.text.len())?;
                text = change.text;
            }
            lines = LineIndex::new(&text);
        }
        Ok((text, lines))
    }

    pub fn validate_position(&self, position: Position) -> Result<(), ResponseError> {
        self.ensure_synchronized()?;
        self.lines.byte_offset(&self.text, position).map(|_| ())
    }

    pub fn ast(&mut self, parser: &YozoraParser) -> Result<&Root, ResponseError> {
        self.ensure_synchronized()?;
        Ok(self.ast.get_or_insert_with(|| {
            Arc::new(parser.parse(
                self.text.as_str(),
                Some(ParseOptions {
                    should_reserve_position: Some(true),
                    ..ParseOptions::default()
                }),
            ))
        }))
    }

    /// Text identity also distinguishes close/reopen when the client reuses a
    /// version number. Only the live owner may adopt a worker's cache.
    pub fn matches(&self, snapshot: &Self) -> bool {
        self.synchronized
            && snapshot.synchronized
            && self.version == snapshot.version
            && Arc::ptr_eq(&self.text, &snapshot.text)
    }

    pub fn adopt_ast(&mut self, snapshot: &mut Self) {
        if self.ast.is_none() && self.matches(snapshot) {
            self.ast = snapshot.ast.take();
        }
    }

    /// An immutable source/AST view for edits against this exact version.
    pub fn snapshot(
        &mut self,
        parser: &YozoraParser,
        position: Position,
    ) -> Result<Snapshot<'_>, ResponseError> {
        self.validate_position(position)?;
        self.ast(parser)?;
        Ok(Snapshot {
            root: self.ast.as_ref().expect("AST was initialized"),
            text: &self.text,
            lines: &self.lines,
            version: self.version,
        })
    }

    fn ensure_synchronized(&self) -> Result<(), ResponseError> {
        if self.synchronized {
            Ok(())
        } else {
            Err(ResponseError::new(
                -32801,
                "document is out of sync; reopen it or send a full replacement",
            ))
        }
    }
}

/// Strictly separated descending edits only address an unchanged prefix. Build
/// their output once; all other batches keep LSP's sequential coordinate rules.
/// The strict gap matters for clamping and edits that join CR and LF boundaries.
fn apply_descending_changes(
    text: &str,
    lines: &LineIndex,
    changes: &[ContentChange],
) -> Result<Option<String>, ResponseError> {
    if changes.len() < 2 {
        return Ok(None);
    }
    let mut boundary = text.len() + 1;
    let mut length = text.len();
    let mut edits = Vec::with_capacity(changes.len());
    for change in changes {
        let Some(range) = change.range else {
            return Ok(None);
        };
        if range.start > range.end {
            return Ok(None);
        }
        let (Ok(start), Ok(end)) = (
            lines.byte_offset(text, range.start),
            lines.byte_offset(text, range.end),
        ) else {
            // Earlier edits may have created these positions. Let the general
            // path resolve them against each intermediate document.
            return Ok(None);
        };
        if end >= boundary {
            return Ok(None);
        }
        boundary = start;
        length = length - (end - start) + change.text.len();
        // Reject an oversized intermediate state even if a later edit shrinks it.
        check_size(length)?;
        edits.push((start, end, change.text.as_str()));
    }
    let mut result = String::with_capacity(length);
    let mut start = 0;
    for (edit_start, edit_end, replacement) in edits.into_iter().rev() {
        result.push_str(&text[start..edit_start]);
        result.push_str(replacement);
        start = edit_end;
    }
    result.push_str(&text[start..]);
    Ok(Some(result))
}

pub(super) fn check_size(bytes: usize) -> Result<(), ResponseError> {
    if bytes > MAX_DOCUMENT_BYTES {
        Err(ResponseError::invalid_params("document exceeds 16 MiB"))
    } else {
        Ok(())
    }
}

#[derive(Clone)]
pub struct LineIndex {
    starts: Vec<usize>,
    checkpoints: Vec<LineCheckpoint>,
}

#[derive(Clone)]
struct LineCheckpoint {
    byte_offset: usize,
    position: Position,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        let mut checkpoints = Vec::new();
        let mut characters = text.char_indices().peekable();
        let mut character = 0;
        let mut checkpoint_offset = 0;
        while let Some((offset, current)) = characters.next() {
            if matches!(current, '\r' | '\n') {
                let mut next = offset + 1;
                if current == '\r' && characters.peek().is_some_and(|(_, next)| *next == '\n') {
                    characters.next();
                    next += 1;
                }
                starts.push(next);
                character = 0;
                checkpoint_offset = next;
                continue;
            }
            // Bound scans in both directions on long lines, without allocating
            // an entry per character or any checkpoints for short lines.
            if offset - checkpoint_offset >= LINE_CHECKPOINT_BYTES {
                checkpoints.push(LineCheckpoint {
                    byte_offset: offset,
                    position: Position {
                        line: (starts.len() - 1) as u32,
                        character,
                    },
                });
                checkpoint_offset = offset;
            }
            character += current.len_utf16() as u32;
        }
        Self {
            starts,
            checkpoints,
        }
    }

    pub fn byte_offset(&self, text: &str, position: Position) -> Result<usize, ResponseError> {
        let line = position.line as usize;
        let start = *self
            .starts
            .get(line)
            .ok_or_else(|| ResponseError::invalid_params("line is outside the document"))?;
        let end = self.starts.get(line + 1).copied().unwrap_or(text.len());
        let end = start + text[start..end].trim_end_matches(['\r', '\n']).len();
        let checkpoint = self
            .checkpoints
            .partition_point(|checkpoint| checkpoint.position <= position)
            .checked_sub(1)
            .map(|index| &self.checkpoints[index])
            .filter(|checkpoint| checkpoint.position.line == position.line);
        let (start, mut units) = checkpoint
            .map(|checkpoint| (checkpoint.byte_offset, checkpoint.position.character))
            .unwrap_or((start, 0));
        let content = &text[start..end];
        for (offset, character) in content.char_indices() {
            if units == position.character {
                return Ok(start + offset);
            }
            units += character.len_utf16() as u32;
            if units > position.character {
                return Err(ResponseError::invalid_params(
                    "position splits a UTF-16 surrogate pair",
                ));
            }
        }
        // LSP positions beyond the line's content are clamped to its end.
        Ok(start + content.len())
    }

    pub fn position(&self, text: &str, offset: usize) -> Result<Position, ResponseError> {
        if !text.is_char_boundary(offset) {
            return Err(ResponseError::invalid_params(
                "offset splits a UTF-8 character",
            ));
        }
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        let checkpoint = self
            .checkpoints
            .partition_point(|checkpoint| checkpoint.byte_offset <= offset)
            .checked_sub(1)
            .map(|index| &self.checkpoints[index])
            .filter(|checkpoint| checkpoint.position.line as usize == line);
        let (start, character) = checkpoint
            .map(|checkpoint| (checkpoint.byte_offset, checkpoint.position.character))
            .unwrap_or((self.starts[line], 0));
        Ok(Position {
            line: line as u32,
            character: character + text[start..offset].encode_utf16().count() as u32,
        })
    }
}

pub(super) fn open_document<'a>(
    documents: &'a mut HashMap<String, Document>,
    uri: &str,
) -> Result<&'a mut Document, ResponseError> {
    documents
        .get_mut(uri)
        .ok_or_else(|| ResponseError::invalid_params("document is not open"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Range;

    fn position(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn change(start: Position, end: Position, text: &str) -> ContentChange {
        ContentChange {
            range: Some(Range { start, end }),
            text: text.to_string(),
        }
    }

    #[test]
    fn worker_snapshots_share_data_and_only_return_caches_to_the_same_live_revision() {
        let parser = YozoraParser::default();
        let mut live = Document::new(1, "# Before".to_string()).unwrap();
        let mut worker = live.clone();
        assert!(live.matches(&worker));
        assert!(Arc::ptr_eq(&live.lines, &worker.lines));
        worker.ast(&parser).unwrap();
        assert!(live.ast.is_none());
        live.adopt_ast(&mut worker);
        assert!(live.ast.is_some());
        assert!(worker.ast.is_none());
        let mut stale = live.clone();
        live.change(
            2,
            vec![ContentChange {
                range: None,
                text: "# After".to_string(),
            }],
        )
        .unwrap();
        assert!(!live.matches(&stale));
        live.adopt_ast(&mut stale);
        assert!(live.ast.is_none());
        assert!(stale.ast.is_some());
        assert_eq!(
            stale.snapshot(&parser, Position::default()).unwrap().text,
            "# Before"
        );
        let mut reopened = Document::new(1, "# Before".to_string()).unwrap();
        reopened.adopt_ast(&mut stale);
        assert!(reopened.ast.is_none());
        assert!(!reopened.matches(&stale));
    }

    #[test]
    fn indexes_utf16_crlf_cr_tabs_and_the_final_empty_line() {
        let text = "中😀\r\na\tb\rx\n";
        let lines = LineIndex::new(text);
        for (point, expected) in [
            (position(0, 1), 3),
            (position(0, 3), 7),
            (position(0, 99), 7),
            (position(1, 0), 9),
            (position(1, 2), 11),
            (position(2, 0), 13),
            (position(3, 0), 15),
        ] {
            assert_eq!(lines.byte_offset(text, point).unwrap(), expected);
        }
        assert!(lines.byte_offset(text, position(0, 2)).is_err());
        assert!(lines.byte_offset(text, position(4, 0)).is_err());
    }

    #[test]
    fn round_trips_long_unicode_lines_and_rejects_split_characters() {
        let contents = [
            ("a中😀\t".repeat(100), "\r\n"),
            ("short".into(), "\r"),
            ("😀𐀀界".repeat(100), "\n"),
            ("x".repeat(1_000), "\n"),
            (String::new(), ""),
        ];
        let text: String = contents
            .iter()
            .map(|(line, ending)| format!("{line}{ending}"))
            .collect();
        let lines = LineIndex::new(&text);
        let mut line_start = 0;
        for (line, (content, ending)) in contents.iter().enumerate() {
            let mut units = 0;
            for (offset, character) in content.char_indices() {
                let expected = position(line as u32, units);
                assert_eq!(
                    lines.byte_offset(&text, expected).unwrap(),
                    line_start + offset
                );
                assert_eq!(
                    lines.position(&text, line_start + offset).unwrap(),
                    expected
                );
                if character.len_utf16() == 2 {
                    assert!(lines
                        .byte_offset(&text, position(line as u32, units + 1))
                        .is_err());
                }
                for byte in 1..character.len_utf8() {
                    assert!(lines.position(&text, line_start + offset + byte).is_err());
                }
                units += character.len_utf16() as u32;
            }
            let end = line_start + content.len();
            assert_eq!(
                lines.position(&text, end).unwrap(),
                position(line as u32, units)
            );
            assert_eq!(
                lines
                    .byte_offset(&text, position(line as u32, u32::MAX))
                    .unwrap(),
                end
            );
            line_start = end + ending.len();
        }
    }

    #[test]
    fn applies_changes_sequentially_and_rebuilds_the_ast() {
        let parser = YozoraParser::default();
        let mut document = Document::new(1, "# 😀\r\nbody".into()).unwrap();
        document.ast(&parser).unwrap();
        assert!(document.ast.is_some());
        document
            .change(
                2,
                vec![
                    change(position(0, 2), position(0, 4), "标题"),
                    change(position(0, 4), position(1, 0), "\n## Child\n"),
                    change(position(2, 0), position(2, 4), "changed"),
                ],
            )
            .unwrap();
        assert_eq!(document.text.as_str(), "# 标题\n## Child\nchanged");
        assert!(document.ast.is_none());
        assert_eq!(document.ast(&parser).unwrap().children.len(), 3);
        assert!(document.change(2, Vec::new()).is_err());
        assert!(document.ast.is_some());
    }

    #[test]
    fn batches_descending_edits_with_unicode_and_newlines() {
        let source = "a😀oldz\r\n".repeat(8_000);
        let changes: Vec<_> = (0..8_000)
            .rev()
            .map(|line| change(position(line, 3), position(line, 6), "renamed\n中"))
            .collect();
        let mut document = Document::new(1, source.clone()).unwrap();
        let expected = source.replace("old", "renamed\n中");
        assert_eq!(
            apply_descending_changes(&source, &document.lines, &changes).unwrap(),
            Some(expected.clone())
        );
        document.change(2, changes).unwrap();
        assert_eq!(document.text.as_str(), expected);
        assert_eq!(document.lines.starts.len(), 16_001);
        let last = document
            .lines
            .byte_offset(&document.text, position(15_999, 1))
            .unwrap();
        assert_eq!(&document.text[last..], "z\r\n");

        let mut document = Document::new(1, "A\nB\nC\nD\nE".into()).unwrap();
        document
            .change(
                2,
                vec![
                    change(position(3, 0), position(4, 1), "尾\r\nZ"),
                    change(position(0, 0), position(1, 1), "首"),
                ],
            )
            .unwrap();
        assert_eq!(document.text.as_str(), "首\nC\n尾\r\nZ");
    }

    #[test]
    fn touching_and_overlapping_edits_keep_sequential_coordinates() {
        for (source, changes, expected) in [
            (
                "\rx\n",
                vec![
                    change(position(1, 0), position(1, 1), ""),
                    change(position(1, 0), position(1, 0), "Y"),
                ],
                "\r\nY",
            ),
            (
                "abc",
                vec![
                    change(position(0, 99), position(0, 99), "X"),
                    change(position(0, 99), position(0, 99), "Y"),
                ],
                "abcXY",
            ),
            (
                "abcd",
                vec![
                    change(position(0, 2), position(0, 4), "XYZ"),
                    change(position(0, 1), position(0, 3), "?"),
                ],
                "a?YZ",
            ),
        ] {
            let mut document = Document::new(1, source.into()).unwrap();
            assert!(apply_descending_changes(source, &document.lines, &changes)
                .unwrap()
                .is_none());
            document.change(2, changes).unwrap();
            assert_eq!(document.text.as_str(), expected);
        }
    }

    #[test]
    fn edit_batches_match_the_sequential_oracle() {
        // Reproducible small-state exploration includes invalid positions,
        // surrogate splits, clamping, CRLF joins and mixed full replacements.
        fn next(seed: &mut u64, limit: usize) -> usize {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            *seed as usize % limit
        }
        let mut seed = 0x8ead_14a7_d306_c129;
        let mut batches = 0;
        let replacements = ["", "x", "😀", "\r", "\n", "\r\n", "中\n😀\r"];
        for source in [
            "",
            "abc",
            "\rx\n",
            "a\r\nb\nc\rd",
            "a😀b\n中x\r\nend",
            "😀\r中\nx",
        ] {
            let document = Document::new(1, source.into()).unwrap();
            let mut points: Vec<_> = (0..=source.len())
                .filter(|offset| source.is_char_boundary(*offset))
                .map(|offset| document.lines.position(source, offset).unwrap())
                .collect();
            for line in 0..=document.lines.starts.len() as u32 {
                points.extend([position(line, 2), position(line, u32::MAX)]);
            }
            for _ in 0..4_000 {
                let changes: Vec<_> = (0..2 + next(&mut seed, 3))
                    .map(|_| {
                        let mut start = points[next(&mut seed, points.len())];
                        let mut end = points[next(&mut seed, points.len())];
                        if start > end && next(&mut seed, 4) != 0 {
                            std::mem::swap(&mut start, &mut end);
                        }
                        ContentChange {
                            range: (next(&mut seed, 12) != 0).then_some(Range { start, end }),
                            text: replacements[next(&mut seed, replacements.len())].into(),
                        }
                    })
                    .collect();
                if apply_descending_changes(source, &document.lines, &changes)
                    .unwrap()
                    .is_some()
                {
                    batches += 1;
                }
                let actual = document
                    .apply_changes(changes.clone())
                    .map(|(text, _)| text)
                    .map_err(|error| (error.code, error.message));
                let expected = document
                    .apply_sequential_changes(changes.clone())
                    .map(|(text, _)| text)
                    .map_err(|error| (error.code, error.message));
                assert_eq!(actual, expected, "source={source:?}, changes={changes:?}");
            }
        }
        assert!(batches > 100, "descending batches exercised: {batches}");
    }

    #[test]
    fn rejects_oversized_intermediate_batches_atomically() {
        let source = format!("a\n{}", "x".repeat(MAX_DOCUMENT_BYTES - 2));
        let mut document = Document::new(1, source.clone()).unwrap();
        let changes = vec![
            change(position(1, 0), position(1, 0), "z"),
            change(position(0, 0), position(0, 1), ""),
        ];
        assert_eq!(document.change(2, changes).unwrap_err().code, -32602);
        assert_eq!(document.text.as_str(), source);
        assert!(!document.synchronized);
        document
            .change(
                3,
                vec![
                    ContentChange {
                        range: None,
                        text: "a\nb\nc".into(),
                    },
                    change(position(2, 0), position(2, 1), "last"),
                    change(position(0, 0), position(0, 1), "first"),
                ],
            )
            .unwrap();
        assert!(document.synchronized);
        assert_eq!(document.text.as_str(), "first\nb\nlast");
    }

    #[test]
    fn rejects_a_partial_batch_and_requires_full_resynchronization() {
        let parser = YozoraParser::default();
        let mut document = Document::new(1, "# 😀".into()).unwrap();
        let result = document.change(
            2,
            vec![
                change(position(0, 0), position(0, 0), "#"),
                change(position(0, 4), position(0, 5), "broken"),
            ],
        );
        assert!(result.is_err());
        assert_eq!(document.text.as_str(), "# 😀");
        assert_eq!(document.ast(&parser).unwrap_err().code, -32801);
        assert!(document
            .change(3, vec![change(position(0, 0), position(0, 1), "x")])
            .is_err());
        document
            .change(
                4,
                vec![ContentChange {
                    range: None,
                    text: "# Recovered".into(),
                }],
            )
            .unwrap();
        assert!(document.ast(&parser).is_ok());
    }

    #[test]
    fn invalidates_only_newer_versions_when_a_batch_cannot_be_decoded() {
        let parser = YozoraParser::default();
        let mut document = Document::new(1, "# Before😀".into()).unwrap();
        document.ast(&parser).unwrap();
        assert!(document.invalidate(1).is_err());
        assert!(document.ast.is_some());
        document.invalidate(2).unwrap();
        assert_eq!(document.version(), 2);
        assert_eq!(document.text.as_str(), "# Before😀");
        assert!(document.ast.is_none());
        assert_eq!(document.ast(&parser).unwrap_err().code, -32801);
        document
            .change(
                3,
                vec![ContentChange {
                    range: None,
                    text: "# Recovered".into(),
                }],
            )
            .unwrap();
        assert_eq!(document.text.as_str(), "# Recovered");
        assert!(document.ast(&parser).is_ok());
    }
}
