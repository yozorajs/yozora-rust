use yozora_ast::HtmlContentType;
use yozora_character::{calc_string_from_node_points, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{eat_optional_whitespaces, NodeInterval, TokenDelimiter};
use yozora_tokenizer_html_block::eat_html_tag_name;

use super::full_delimiter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineClosingTagData {
    pub html_type: HtmlContentType,
    pub tag_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlInlineClosingTokenData {
    pub html_type: HtmlContentType,
    pub tag_name: NodeInterval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineClosingDelimiter {
    pub delimiter: TokenDelimiter,
    pub html_type: HtmlContentType,
    pub tag_name: NodeInterval,
}

impl HtmlInlineClosingDelimiter {
    pub fn to_data(&self, node_points: &[NodePoint]) -> HtmlInlineClosingTagData {
        HtmlInlineClosingTagData {
            html_type: HtmlContentType::Closing,
            tag_name: calc_string_from_node_points(
                node_points,
                self.tag_name.start_index,
                self.tag_name.end_index,
                false,
            ),
        }
    }
}

pub fn eat_html_inline_closing_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineClosingDelimiter> {
    let mut i = start_index;
    if i + 3 >= end_index || node_points[i + 1].code_point != AsciiCodePoint::SLASH as i32 {
        return None;
    }

    let tag_name_start_index = i + 2;
    let tag_name_end_index = eat_html_tag_name(node_points, tag_name_start_index, end_index)?;
    i = eat_optional_whitespaces(node_points, tag_name_end_index, end_index);
    if i >= end_index || node_points[i].code_point != AsciiCodePoint::CLOSE_ANGLE as i32 {
        return None;
    }

    Some(HtmlInlineClosingDelimiter {
        delimiter: full_delimiter(start_index, i + 1),
        html_type: HtmlContentType::Closing,
        tag_name: NodeInterval {
            start_index: tag_name_start_index,
            end_index: tag_name_end_index,
        },
    })
}
