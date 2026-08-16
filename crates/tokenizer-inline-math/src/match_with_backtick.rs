use yozora_ast::INLINE_MATH_TYPE;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    eat_optional_characters, DelimiterType, InlineToken, MatchInlinePhaseApi, TokenDelimiter,
};

use crate::types::InlineMathTokenData;

#[derive(Debug, Clone)]
struct PotentialDelimiter {
    delimiter_type: DelimiterType,
    start_index: usize,
    end_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct InlineMathBacktickDelimiterFinder {
    potential_delimiters: Vec<PotentialDelimiter>,
    cursor: usize,
}

impl InlineMathBacktickDelimiterFinder {
    pub(crate) fn new(api: &dyn MatchInlinePhaseApi) -> Self {
        let potential_delimiters = collect_potential_delimiters(
            api.get_node_points(),
            api.get_block_start_index(),
            api.get_block_end_index(),
        );

        Self {
            potential_delimiters,
            cursor: 0,
        }
    }

    pub(crate) fn find_next_delimiter(&mut self, start_index: usize) -> Option<TokenDelimiter> {
        let len = self.potential_delimiters.len();
        while self.cursor < len {
            while self.cursor < len {
                let delimiter = &self.potential_delimiters[self.cursor];
                if delimiter.start_index >= start_index
                    && delimiter.delimiter_type != DelimiterType::Closer
                {
                    break;
                }
                self.cursor += 1;
            }

            if self.cursor + 1 >= len {
                return None;
            }

            let opener = &self.potential_delimiters[self.cursor];
            let thickness = opener.end_index.saturating_sub(opener.start_index);

            let mut closer: Option<&PotentialDelimiter> = None;
            for i in (self.cursor + 1)..len {
                let delimiter = &self.potential_delimiters[i];
                if delimiter.delimiter_type != DelimiterType::Opener
                    && delimiter.end_index.saturating_sub(delimiter.start_index) == thickness
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

pub(crate) fn process_single_delimiter(delimiter: &TokenDelimiter) -> Vec<InlineToken> {
    vec![InlineToken::new(
        "",
        INLINE_MATH_TYPE,
        (delimiter.start_index, delimiter.end_index),
    )
    .with_data(InlineMathTokenData {
        thickness: delimiter.thickness,
    })]
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
            i += 2;
            continue;
        }

        if c == AsciiCodePoint::BACKTICK as i32 {
            let start_index = i;
            let j = eat_optional_characters(
                node_points,
                i + 1,
                block_end_index,
                AsciiCodePoint::BACKTICK as i32,
            );

            if j < block_end_index
                && node_points[j].code_point == AsciiCodePoint::DOLLAR_SIGN as i32
            {
                delimiters.push(PotentialDelimiter {
                    delimiter_type: DelimiterType::Opener,
                    start_index,
                    end_index: j + 1,
                });
            }

            i = j;
            continue;
        }

        if c == AsciiCodePoint::DOLLAR_SIGN as i32 {
            let start_index = i;
            let j = eat_optional_characters(
                node_points,
                i + 1,
                block_end_index,
                AsciiCodePoint::BACKTICK as i32,
            );
            let thickness = j.saturating_sub(start_index);

            if thickness > 1
                && (j >= block_end_index
                    || node_points[j].code_point != AsciiCodePoint::DOLLAR_SIGN as i32)
            {
                delimiters.push(PotentialDelimiter {
                    delimiter_type: DelimiterType::Closer,
                    start_index,
                    end_index: j,
                });
            }

            i = j;
            continue;
        }

        i += 1;
    }

    delimiters
}
