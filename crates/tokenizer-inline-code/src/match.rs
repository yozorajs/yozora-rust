use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{DelimiterType, MatchInlinePhaseApi, TokenDelimiter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PotentialDelimiterType {
    Opener,
    Closer,
    Both,
}

#[derive(Debug, Clone, Copy)]
struct PotentialDelimiter {
    delimiter_type: PotentialDelimiterType,
    start_index: usize,
    end_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct InlineCodeDelimiterFinder {
    potential_delimiters: Vec<PotentialDelimiter>,
    cursor: usize,
}

impl InlineCodeDelimiterFinder {
    pub(crate) fn new(api: &dyn MatchInlinePhaseApi) -> Self {
        let node_points = api.get_node_points();
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();

        Self {
            potential_delimiters: collect_potential_delimiters(
                node_points,
                block_start_index,
                block_end_index,
            ),
            cursor: 0,
        }
    }

    pub(crate) fn find_next_delimiter(&mut self, start_index: usize) -> Option<TokenDelimiter> {
        let len = self.potential_delimiters.len();
        while self.cursor < len {
            while self.cursor < len {
                let delimiter = self.potential_delimiters[self.cursor];
                if delimiter.start_index >= start_index
                    && delimiter.delimiter_type != PotentialDelimiterType::Closer
                {
                    break;
                }
                self.cursor += 1;
            }

            if self.cursor + 1 >= len {
                return None;
            }

            let opener = self.potential_delimiters[self.cursor];
            let thickness = opener.end_index - opener.start_index;

            let mut closer: Option<PotentialDelimiter> = None;
            for i in (self.cursor + 1)..len {
                let delimiter = self.potential_delimiters[i];
                if delimiter.delimiter_type != PotentialDelimiterType::Opener
                    && delimiter.end_index - delimiter.start_index == thickness
                {
                    closer = Some(delimiter);
                    break;
                }
            }

            if let Some(closer) = closer {
                return Some(TokenDelimiter {
                    delimiter_type: DelimiterType::Full,
                    start_index: opener.start_index,
                    end_index: closer.end_index,
                    thickness,
                    original_thickness: thickness,
                });
            }

            self.cursor += 1;
        }

        None
    }
}

fn collect_potential_delimiters(
    node_points: &[NodePoint],
    block_start_index: usize,
    block_end_index: usize,
) -> Vec<PotentialDelimiter> {
    let mut delimiters = Vec::new();
    let mut i = block_start_index;

    while i < block_end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::BACKSLASH as i32 {
            i += 1;
            if i < block_end_index && node_points[i].code_point == AsciiCodePoint::BACKTICK as i32 {
                let j = eat_optional_characters(
                    node_points,
                    i + 1,
                    block_end_index,
                    AsciiCodePoint::BACKTICK as i32,
                );

                delimiters.push(PotentialDelimiter {
                    delimiter_type: PotentialDelimiterType::Closer,
                    start_index: i,
                    end_index: j,
                });

                if j > i + 1 {
                    delimiters.push(PotentialDelimiter {
                        delimiter_type: PotentialDelimiterType::Opener,
                        start_index: i + 1,
                        end_index: j,
                    });
                }

                i = j;
                continue;
            }

            continue;
        }

        if c == AsciiCodePoint::BACKTICK as i32 {
            let start_index = i;
            let end_index = eat_optional_characters(
                node_points,
                i + 1,
                block_end_index,
                AsciiCodePoint::BACKTICK as i32,
            );

            delimiters.push(PotentialDelimiter {
                delimiter_type: PotentialDelimiterType::Both,
                start_index,
                end_index,
            });

            i = end_index;
            continue;
        }

        i += 1;
    }

    delimiters
}

fn eat_optional_characters(
    node_points: &[NodePoint],
    mut start_index: usize,
    end_index: usize,
    code_point: i32,
) -> usize {
    while start_index < end_index && node_points[start_index].code_point == code_point {
        start_index += 1;
    }
    start_index
}
