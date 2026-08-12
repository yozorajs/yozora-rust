use yozora_ast::HtmlContentType;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::TokenDelimiter;

use super::full_delimiter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineInstructionData {
    pub html_type: HtmlContentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineInstructionTokenData {
    pub html_type: HtmlContentType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineInstructionDelimiter {
    pub delimiter: TokenDelimiter,
    pub html_type: HtmlContentType,
}

pub fn eat_html_inline_instruction_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineInstructionDelimiter> {
    let mut i = start_index;
    if i + 3 >= end_index || node_points[i + 1].code_point != AsciiCodePoint::QUESTION_MARK as i32 {
        return None;
    }

    i += 2;
    while i < end_index {
        if node_points[i].code_point == AsciiCodePoint::QUESTION_MARK as i32 {
            if i + 1 >= end_index {
                return None;
            }
            if node_points[i + 1].code_point == AsciiCodePoint::CLOSE_ANGLE as i32 {
                return Some(HtmlInlineInstructionDelimiter {
                    delimiter: full_delimiter(start_index, i + 2),
                    html_type: HtmlContentType::Instruction,
                });
            }
        }
        i += 1;
    }
    None
}
