use yozora_ast::HtmlContentType;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::TokenDelimiter;

use super::full_delimiter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineCDataData {
    pub html_type: HtmlContentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineCDataTokenData {
    pub html_type: HtmlContentType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineCDataDelimiter {
    pub delimiter: TokenDelimiter,
    pub html_type: HtmlContentType,
}

pub fn eat_html_inline_cdata_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineCDataDelimiter> {
    let mut i = start_index;
    if i + 11 >= end_index
        || node_points[i + 1].code_point != AsciiCodePoint::EXCLAMATION_MARK as i32
        || node_points[i + 2].code_point != AsciiCodePoint::OPEN_BRACKET as i32
        || node_points[i + 3].code_point != AsciiCodePoint::UPPERCASE_C as i32
        || node_points[i + 4].code_point != AsciiCodePoint::UPPERCASE_D as i32
        || node_points[i + 5].code_point != AsciiCodePoint::UPPERCASE_A as i32
        || node_points[i + 6].code_point != AsciiCodePoint::UPPERCASE_T as i32
        || node_points[i + 7].code_point != AsciiCodePoint::UPPERCASE_A as i32
        || node_points[i + 8].code_point != AsciiCodePoint::OPEN_BRACKET as i32
    {
        return None;
    }

    i += 9;
    while i < end_index {
        if node_points[i].code_point == AsciiCodePoint::CLOSE_BRACKET as i32 {
            if i + 2 >= end_index {
                return None;
            }
            if node_points[i + 1].code_point == AsciiCodePoint::CLOSE_BRACKET as i32
                && node_points[i + 2].code_point == AsciiCodePoint::CLOSE_ANGLE as i32
            {
                return Some(HtmlInlineCDataDelimiter {
                    delimiter: full_delimiter(start_index, i + 3),
                    html_type: HtmlContentType::Cdata,
                });
            }
        }
        i += 1;
    }
    None
}
