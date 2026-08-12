use yozora_ast::HtmlContentType;
use yozora_character::{is_ascii_upper_letter, is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{NodeInterval, TokenDelimiter};

use super::full_delimiter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineDeclarationData {
    pub html_type: HtmlContentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineDeclarationTokenData {
    pub html_type: HtmlContentType,
    pub tag_name: NodeInterval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineDeclarationDelimiter {
    pub delimiter: TokenDelimiter,
    pub html_type: HtmlContentType,
    pub tag_name: NodeInterval,
}

pub fn eat_html_inline_declaration_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineDeclarationDelimiter> {
    let mut i = start_index;
    if i + 4 >= end_index
        || node_points[i + 1].code_point != AsciiCodePoint::EXCLAMATION_MARK as i32
    {
        return None;
    }

    let tag_name_start_index = i + 2;
    i = tag_name_start_index;
    while i < end_index && is_ascii_upper_letter(node_points[i].code_point) {
        i += 1;
    }
    if i == tag_name_start_index
        || i + 1 >= end_index
        || !is_whitespace_character(node_points[i].code_point)
    {
        return None;
    }

    let tag_name_end_index = i;
    i += 1;
    while i < end_index {
        if node_points[i].code_point == AsciiCodePoint::CLOSE_ANGLE as i32 {
            return Some(HtmlInlineDeclarationDelimiter {
                delimiter: full_delimiter(start_index, i + 1),
                html_type: HtmlContentType::Declaration,
                tag_name: NodeInterval {
                    start_index: tag_name_start_index,
                    end_index: tag_name_end_index,
                },
            });
        }
        i += 1;
    }
    None
}
