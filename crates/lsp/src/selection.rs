use std::cmp::Reverse;

use crate::analysis::{nodes, section_ranges};
use crate::cancellation::Cancellation;
use crate::document::Snapshot;
use crate::protocol::{Position, Range, ResponseError, SelectionRange};

const MAX_DEPTH: usize = 32;

/// Keep the innermost candidates and the document root. This bounds both the
/// response's JSON nesting and per-cursor memory, including pathological ASTs.
pub fn ranges(
    snapshot: &Snapshot<'_>,
    positions: &[Position],
    cancellation: &Cancellation,
) -> Result<Vec<SelectionRange>, ResponseError> {
    let document = Range {
        start: Position::default(),
        end: snapshot
            .lines
            .position(snapshot.text, snapshot.text.len())?,
    };
    let mut points = Vec::with_capacity(positions.len());
    let mut candidates = vec![Vec::new(); positions.len()];
    for (index, &position) in positions.iter().enumerate() {
        cancellation.check()?;
        let offset = snapshot.lines.byte_offset(snapshot.text, position)?;
        let position = snapshot.lines.position(snapshot.text, offset)?;
        points.push((position, index));
        let (line, cursor) = snapshot.line(position)?;
        insert(
            &mut candidates[index],
            Range {
                start: Position {
                    line: position.line,
                    character: 0,
                },
                end: Position {
                    line: position.line,
                    character: line.encode_utf16().count() as u32,
                },
            },
        );
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let start = line[..cursor]
            .char_indices()
            .rev()
            .take_while(|(_, c)| word(*c))
            .last()
            .map_or(cursor, |(offset, _)| offset);
        let end = line[cursor..]
            .char_indices()
            .take_while(|(_, c)| word(*c))
            .last()
            .map_or(cursor, |(offset, c)| cursor + offset + c.len_utf8());
        if start < end {
            insert(
                &mut candidates[index],
                Range {
                    start: snapshot
                        .lines
                        .position(snapshot.text, offset - cursor + start)?,
                    end: snapshot
                        .lines
                        .position(snapshot.text, offset - cursor + end)?,
                },
            );
        }
    }
    points.sort_unstable_by_key(|&(position, _)| position);
    let structural = nodes(&snapshot.root.children)
        .filter_map(|node| node.position().map(Range::from))
        .chain(section_ranges(snapshot.root));
    for range in structural {
        cancellation.check()?;
        if range.start > range.end || range.end > document.end {
            continue;
        }
        let start = points.partition_point(|&(position, _)| position < range.start);
        for &(_, index) in points[start..]
            .iter()
            .take_while(|&&(position, _)| position <= range.end)
        {
            insert(&mut candidates[index], range);
        }
    }
    Ok(candidates
        .into_iter()
        .map(|ranges| {
            let mut chain = Vec::new();
            for range in ranges {
                if chain
                    .last()
                    .is_none_or(|child: &Range| contains(range, *child))
                {
                    chain.push(range);
                }
            }
            if chain.last() != Some(&document) {
                chain.push(document);
            }
            let mut parent = None;
            for range in chain.into_iter().rev() {
                parent = Some(Box::new(SelectionRange { range, parent }));
            }
            *parent.expect("the document always supplies a selection range")
        })
        .collect())
}

fn contains(parent: Range, child: Range) -> bool {
    parent.start <= child.start && child.end <= parent.end
}

fn insert(ranges: &mut Vec<Range>, range: Range) {
    let key = (Reverse(range.start), range.end);
    if let Err(index) = ranges.binary_search_by_key(&key, |range| (Reverse(range.start), range.end))
    {
        if index < MAX_DEPTH - 1 {
            ranges.insert(index, range);
            ranges.truncate(MAX_DEPTH - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use yozora_parser::YozoraParser;

    fn selections(text: &str, positions: &[Position]) -> Vec<Vec<Range>> {
        let mut document = Document::new(1, text.into()).unwrap();
        let parser = YozoraParser::default();
        let snapshot = document.snapshot(&parser, Position::default()).unwrap();
        ranges(&snapshot, positions, &Cancellation::default())
            .unwrap()
            .into_iter()
            .map(|selection| {
                let mut chain = Vec::new();
                let mut next = Some(selection);
                while let Some(selection) = next {
                    chain.push(selection.range);
                    next = selection.parent.map(|parent| *parent);
                }
                assert!(chain.len() <= MAX_DEPTH);
                assert!(chain
                    .windows(2)
                    .all(|pair| pair[0] != pair[1] && contains(pair[1], pair[0])));
                chain
            })
            .collect()
    }

    #[test]
    fn expands_inline_content_blocks_and_heading_sections_in_request_order() {
        let text = "# Title\r\n\r\n> a **中文😀 word**\r\n> next\r\n\r\n## Child\r\nbody\r\n";
        let positions = [
            Position {
                line: 6,
                character: 2,
            },
            Position {
                line: 2,
                character: 13,
            },
        ];
        let result = selections(text, &positions);
        assert_eq!(
            result[0][0],
            Range {
                start: Position {
                    line: 6,
                    character: 0
                },
                end: Position {
                    line: 6,
                    character: 4
                }
            }
        );
        assert_eq!(
            result[1][0],
            Range {
                start: Position {
                    line: 2,
                    character: 11
                },
                end: Position {
                    line: 2,
                    character: 15
                }
            }
        );
        assert!(result[1]
            .iter()
            .any(|range| range.start.line == 2 && (3..=4).contains(&range.end.line)));
        assert_eq!(result[0].last(), result[1].last());
        assert!(result[0]
            .iter()
            .any(|range| range.start.line == 5 && range.end.line >= 6));
    }

    #[test]
    fn empty_lines_eof_duplicate_positions_and_deep_nesting_remain_bounded() {
        let point = Position::default();
        assert_eq!(
            selections("", &[point, point]),
            vec![
                vec![Range {
                    start: point,
                    end: point
                }];
                2
            ]
        );
        assert!(
            selections(
                "\n",
                &[Position {
                    line: 1,
                    character: 0
                }]
            )[0]
            .len()
                <= 2
        );
        let text = format!("{}word{}", "![".repeat(200), "](x)".repeat(200));
        let result = selections(
            &text,
            &[Position {
                line: 0,
                character: 402,
            }],
        );
        assert_eq!(result[0].last().unwrap().end.character as usize, text.len());
    }

    #[test]
    fn rejects_invalid_utf16_and_observes_cancellation() {
        let parser = YozoraParser::default();
        let mut document = Document::new(1, "😀".into()).unwrap();
        let snapshot = document.snapshot(&parser, Position::default()).unwrap();
        assert!(ranges(
            &snapshot,
            &[Position {
                line: 0,
                character: 1
            }],
            &Cancellation::default()
        )
        .is_err());
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(ranges(&snapshot, &[Position::default()], &cancellation).is_err());
    }
}
