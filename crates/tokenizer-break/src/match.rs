use yozora_character::{AsciiCodePoint, VirtualCodePoint};
use yozora_core_tokenizer::{DelimiterType, MatchInlinePhaseApi, TokenDelimiter};

pub(crate) fn find_break_delimiter(
    api: &dyn MatchInlinePhaseApi,
    start_index: usize,
    end_index: usize,
) -> Option<TokenDelimiter> {
    let node_points = api.get_node_points();
    if start_index + 1 >= end_index || end_index > node_points.len() {
        return None;
    }

    for i in (start_index + 1)..end_index {
        if node_points[i].code_point != VirtualCodePoint::LineEnd as i32 {
            continue;
        }

        let prev = node_points[i - 1].code_point;
        let marker_start = if prev == AsciiCodePoint::BACKSLASH as i32 {
            let mut x = i.saturating_sub(2) as isize;
            while x >= start_index as isize
                && node_points[x as usize].code_point == AsciiCodePoint::BACKSLASH as i32
            {
                x -= 1;
            }

            if ((i as isize - x) & 1) == 0 {
                Some(i - 1)
            } else {
                None
            }
        } else if prev == AsciiCodePoint::SPACE as i32 {
            let mut x = i.saturating_sub(2) as isize;
            while x >= start_index as isize
                && node_points[x as usize].code_point == AsciiCodePoint::SPACE as i32
            {
                x -= 1;
            }

            if i as isize - x > 2 {
                Some((x + 1) as usize)
            } else {
                None
            }
        } else {
            None
        };

        let Some(marker_start) = marker_start else {
            continue;
        };

        return Some(TokenDelimiter {
            delimiter_type: DelimiterType::Full,
            start_index: marker_start,
            end_index: i + 1,
            thickness: i + 1 - marker_start,
            original_thickness: i + 1 - marker_start,
        });
    }

    None
}
