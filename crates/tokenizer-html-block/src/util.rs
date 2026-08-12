use yozora_character::{
    is_ascii_digit_character, is_ascii_letter, is_whitespace_character, AsciiCodePoint, NodePoint,
};
use yozora_core_tokenizer::{eat_optional_whitespaces, NodeInterval};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawHtmlAttribute {
    pub name: NodeInterval,
    pub value: Option<NodeInterval>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EatHtmlAttributeResult {
    pub attribute: RawHtmlAttribute,
    pub next_index: usize,
}

pub fn eat_html_attribute(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<EatHtmlAttributeResult> {
    let mut i = eat_optional_whitespaces(node_points, start_index, end_index);
    if i <= start_index || i >= end_index {
        return None;
    }

    let attr_name_start_index = i;
    let mut code_point = node_points[i].code_point;
    if !is_ascii_letter(code_point)
        && code_point != AsciiCodePoint::UNDERSCORE as i32
        && code_point != AsciiCodePoint::COLON as i32
    {
        return None;
    }

    i = attr_name_start_index + 1;
    while i < end_index {
        code_point = node_points[i].code_point;
        if is_ascii_letter(code_point)
            || is_ascii_digit_character(code_point)
            || code_point == AsciiCodePoint::UNDERSCORE as i32
            || code_point == AsciiCodePoint::DOT as i32
            || code_point == AsciiCodePoint::COLON as i32
            || code_point == AsciiCodePoint::MINUS_SIGN as i32
        {
            i += 1;
            continue;
        }
        break;
    }
    let attr_name_end_index = i;
    let mut attribute = RawHtmlAttribute {
        name: NodeInterval {
            start_index: attr_name_start_index,
            end_index: attr_name_end_index,
        },
        value: None,
    };

    i = eat_optional_whitespaces(node_points, attr_name_end_index, end_index);
    if i < end_index && node_points[i].code_point == AsciiCodePoint::EQUALS_SIGN as i32 {
        i = eat_optional_whitespaces(node_points, i + 1, end_index);
        if i < end_index {
            match node_points[i].code_point {
                code_point if code_point == AsciiCodePoint::DOUBLE_QUOTE as i32 => {
                    let attr_value_start_index = i + 1;
                    i = attr_value_start_index;
                    while i < end_index
                        && node_points[i].code_point != AsciiCodePoint::DOUBLE_QUOTE as i32
                    {
                        i += 1;
                    }
                    if i < end_index {
                        attribute.value = Some(NodeInterval {
                            start_index: attr_value_start_index,
                            end_index: i,
                        });
                        i += 1;
                    }
                }
                code_point if code_point == AsciiCodePoint::SINGLE_QUOTE as i32 => {
                    let attr_value_start_index = i + 1;
                    i = attr_value_start_index;
                    while i < end_index
                        && node_points[i].code_point != AsciiCodePoint::SINGLE_QUOTE as i32
                    {
                        i += 1;
                    }
                    if i < end_index {
                        attribute.value = Some(NodeInterval {
                            start_index: attr_value_start_index,
                            end_index: i,
                        });
                        i += 1;
                    }
                }
                _ => {
                    let attr_value_start_index = i;
                    while i < end_index {
                        code_point = node_points[i].code_point;
                        if is_whitespace_character(code_point)
                            || code_point == AsciiCodePoint::DOUBLE_QUOTE as i32
                            || code_point == AsciiCodePoint::SINGLE_QUOTE as i32
                            || code_point == AsciiCodePoint::EQUALS_SIGN as i32
                            || code_point == AsciiCodePoint::OPEN_ANGLE as i32
                            || code_point == AsciiCodePoint::CLOSE_ANGLE as i32
                            || code_point == AsciiCodePoint::BACKTICK as i32
                        {
                            break;
                        }
                        i += 1;
                    }
                    if i > attr_value_start_index {
                        attribute.value = Some(NodeInterval {
                            start_index: attr_value_start_index,
                            end_index: i,
                        });
                    }
                }
            }

            if attribute.value.is_some() {
                return Some(EatHtmlAttributeResult {
                    attribute,
                    next_index: i,
                });
            }
        }
    }

    Some(EatHtmlAttributeResult {
        attribute,
        next_index: attr_name_end_index,
    })
}

pub fn eat_html_tag_name(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<usize> {
    if start_index >= end_index || !is_ascii_letter(node_points[start_index].code_point) {
        return None;
    }

    let mut i = start_index;
    while i < end_index {
        let code_point = node_points[i].code_point;
        if is_ascii_letter(code_point)
            || is_ascii_digit_character(code_point)
            || code_point == AsciiCodePoint::MINUS_SIGN as i32
        {
            i += 1;
            continue;
        }
        return Some(i);
    }
    Some(i)
}
