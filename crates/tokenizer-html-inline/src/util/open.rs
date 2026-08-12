use yozora_ast::HtmlContentType;
use yozora_character::{calc_string_from_node_points, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{eat_optional_whitespaces, NodeInterval, TokenDelimiter};
use yozora_tokenizer_html_block::{eat_html_attribute, eat_html_tag_name, RawHtmlAttribute};

use super::full_delimiter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlAttribute {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineOpenTagData {
    pub html_type: HtmlContentType,
    pub tag_name: String,
    pub attributes: Vec<HtmlAttribute>,
    pub self_closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineOpenTokenData {
    pub html_type: HtmlContentType,
    pub tag_name: NodeInterval,
    pub attributes: Vec<RawHtmlAttribute>,
    pub self_closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlInlineOpenDelimiter {
    pub delimiter: TokenDelimiter,
    pub html_type: HtmlContentType,
    pub tag_name: NodeInterval,
    pub attributes: Vec<RawHtmlAttribute>,
    pub self_closed: bool,
}

impl HtmlInlineOpenDelimiter {
    pub fn to_data(&self, node_points: &[NodePoint]) -> HtmlInlineOpenTagData {
        HtmlInlineOpenTagData {
            html_type: HtmlContentType::Open,
            tag_name: calc_string_from_node_points(
                node_points,
                self.tag_name.start_index,
                self.tag_name.end_index,
                false,
            ),
            attributes: self
                .attributes
                .iter()
                .map(|attribute| HtmlAttribute {
                    name: calc_string_from_node_points(
                        node_points,
                        attribute.name.start_index,
                        attribute.name.end_index,
                        false,
                    ),
                    value: attribute.value.map(|value| {
                        calc_string_from_node_points(
                            node_points,
                            value.start_index,
                            value.end_index,
                            false,
                        )
                    }),
                })
                .collect(),
            self_closed: self.self_closed,
        }
    }
}

pub fn eat_html_inline_token_open_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineOpenDelimiter> {
    let mut i = start_index;
    if i + 2 >= end_index {
        return None;
    }

    let tag_name_start_index = i + 1;
    let tag_name_end_index = eat_html_tag_name(node_points, tag_name_start_index, end_index)?;
    let mut attributes = Vec::new();
    i = tag_name_end_index;
    while i < end_index {
        let Some(result) = eat_html_attribute(node_points, i, end_index) else {
            break;
        };
        attributes.push(result.attribute);
        i = result.next_index;
    }

    i = eat_optional_whitespaces(node_points, i, end_index);
    if i >= end_index {
        return None;
    }

    let mut self_closed = false;
    if node_points[i].code_point == AsciiCodePoint::SLASH as i32 {
        i += 1;
        self_closed = true;
    }
    if i >= end_index || node_points[i].code_point != AsciiCodePoint::CLOSE_ANGLE as i32 {
        return None;
    }

    Some(HtmlInlineOpenDelimiter {
        delimiter: full_delimiter(start_index, i + 1),
        html_type: HtmlContentType::Open,
        tag_name: NodeInterval {
            start_index: tag_name_start_index,
            end_index: tag_name_end_index,
        },
        attributes,
        self_closed,
    })
}
