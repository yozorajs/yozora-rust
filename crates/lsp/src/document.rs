use yozora_ast::Root;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::protocol::{ContentChange, Position, ResponseError};

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const LINE_CHECKPOINT_BYTES: usize = 128;

/// The server is the only writer. Text, line index and AST belong to one version.
/// An invalid newer edit suspends queries until a full replacement restores sync.
pub struct Document {
    version: i32,
    text: String,
    lines: LineIndex,
    synchronized: bool,
    ast: Option<Root>,
}

pub struct Snapshot<'a> {
    pub root: &'a Root,
    pub text: &'a str,
    pub lines: &'a LineIndex,
    pub version: i32,
}

impl Document {
    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn new(version: i32, text: String) -> Result<Self, ResponseError> {
        check_size(text.len())?;
        Ok(Self {
            version,
            lines: LineIndex::new(&text),
            text,
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
                self.text = text;
                self.lines = lines;
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
        let mut text = self.text.clone();
        let mut lines = self.lines.clone();
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
            parser.parse(
                &self.text,
                Some(ParseOptions {
                    should_reserve_position: Some(true),
                    ..ParseOptions::default()
                }),
            )
        }))
    }

    /// Borrow the source line and AST from one synchronized document version.
    /// The cursor is a UTF-8 byte offset within the line, excluding its ending.
    pub fn line_snapshot(
        &mut self,
        parser: &YozoraParser,
        position: Position,
    ) -> Result<(&Root, &str, usize), ResponseError> {
        self.ensure_synchronized()?;
        let cursor = self.lines.byte_offset(&self.text, position)?;
        self.ast(parser)?;
        let start = self.lines.starts[position.line as usize];
        let end = self
            .lines
            .starts
            .get(position.line as usize + 1)
            .copied()
            .unwrap_or(self.text.len());
        Ok((
            self.ast.as_ref().expect("AST was initialized"),
            self.text[start..end].trim_end_matches(['\r', '\n']),
            cursor - start,
        ))
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
        assert_eq!(document.text, "# 标题\n## Child\nchanged");
        assert!(document.ast.is_none());
        assert_eq!(document.ast(&parser).unwrap().children.len(), 3);
        assert!(document.change(2, Vec::new()).is_err());
        assert!(document.ast.is_some());
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
        assert_eq!(document.text, "# 😀");
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
        assert_eq!(document.text, "# Before😀");
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
        assert_eq!(document.text, "# Recovered");
        assert!(document.ast(&parser).is_ok());
    }
}
