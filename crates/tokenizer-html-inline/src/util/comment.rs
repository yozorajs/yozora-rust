use yozora_ast::HtmlContentType;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::TokenDelimiter;

use super::full_delimiter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineCommentData {
    pub html_type: HtmlContentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineCommentTokenData {
    pub html_type: HtmlContentType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineCommentDelimiter {
    pub delimiter: TokenDelimiter,
    pub html_type: HtmlContentType,
}

pub fn eat_html_inline_comment_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineCommentDelimiter> {
    let mut i = start_index;
    if i + 6 >= end_index
        || node_points[i + 1].code_point != AsciiCodePoint::EXCLAMATION_MARK as i32
        || node_points[i + 2].code_point != AsciiCodePoint::MINUS_SIGN as i32
        || node_points[i + 3].code_point != AsciiCodePoint::MINUS_SIGN as i32
    {
        return None;
    }
    if node_points[i + 4].code_point == AsciiCodePoint::CLOSE_ANGLE as i32 {
        return None;
    }
    if node_points[i + 4].code_point == AsciiCodePoint::MINUS_SIGN as i32
        && node_points[i + 5].code_point == AsciiCodePoint::CLOSE_ANGLE as i32
    {
        return None;
    }

    i += 4;
    while i < end_index {
        if node_points[i].code_point != AsciiCodePoint::MINUS_SIGN as i32 {
            i += 1;
            continue;
        }

        let mut hyphen_count = 1usize;
        while i + hyphen_count < end_index
            && node_points[i + hyphen_count].code_point == AsciiCodePoint::MINUS_SIGN as i32
        {
            hyphen_count += 1;
        }
        if hyphen_count < 2 {
            i += 1;
            continue;
        }
        if hyphen_count > 2
            || i + 2 >= end_index
            || node_points[i + 2].code_point != AsciiCodePoint::CLOSE_ANGLE as i32
        {
            return None;
        }
        return Some(HtmlInlineCommentDelimiter {
            delimiter: full_delimiter(start_index, i + 3),
            html_type: HtmlContentType::Comment,
        });
    }
    None
}
