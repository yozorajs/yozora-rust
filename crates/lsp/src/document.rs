use yozora_ast::Root;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

use crate::protocol::{ContentChange, Position, ResponseError};

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

/// The server is the only writer. Text, line index and AST belong to one version.
/// An invalid newer edit suspends queries until a full replacement restores sync.
pub struct Document {
    version: i32,
    text: String,
    lines: LineIndex,
    synchronized: bool,
    ast: Option<Root>,
}

impl Document {
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
        if version <= self.version {
            return Err(ResponseError::invalid_params(
                "document version must increase",
            ));
        }

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

fn check_size(bytes: usize) -> Result<(), ResponseError> {
    if bytes > MAX_DOCUMENT_BYTES {
        Err(ResponseError::invalid_params("document exceeds 16 MiB"))
    } else {
        Ok(())
    }
}

#[derive(Clone)]
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut starts = vec![0];
        let mut offset = 0;
        while offset < bytes.len() {
            match bytes[offset] {
                b'\r' => {
                    offset += 1;
                    if bytes.get(offset) == Some(&b'\n') {
                        offset += 1;
                    }
                    starts.push(offset);
                }
                b'\n' => {
                    offset += 1;
                    starts.push(offset);
                }
                _ => offset += 1,
            }
        }
        Self { starts }
    }

    fn byte_offset(&self, text: &str, position: Position) -> Result<usize, ResponseError> {
        let line = position.line as usize;
        let start = *self
            .starts
            .get(line)
            .ok_or_else(|| ResponseError::invalid_params("line is outside the document"))?;
        let end = self.starts.get(line + 1).copied().unwrap_or(text.len());
        let content = text[start..end].trim_end_matches(['\r', '\n']);
        let mut units = 0;
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
}
