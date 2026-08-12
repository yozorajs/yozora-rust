use yozora_ast::LINK_TYPE;
use yozora_character::{calc_escaped_string_from_node_points, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    eat_optional_whitespaces, DelimiterType, InlineToken, NodeInterval, TokenDelimiter,
};

use crate::parse::LinkTokenData;
use crate::util::{eat_link_destination, eat_link_title};

#[derive(Debug, Clone)]
pub(crate) struct LinkDelimiterData {
    pub destination_content: Option<NodeInterval>,
    pub title_content: Option<NodeInterval>,
}

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
    pub data: Option<LinkDelimiterData>,
}

pub(crate) fn find_link_delimiter_entry(
    node_points: &[NodePoint],
    block_start_index: usize,
    block_end_index: usize,
    start_index: usize,
    end_index: usize,
) -> Option<DelimiterEntry> {
    let mut i = start_index;

    while i < end_index {
        let code_point = node_points[i].code_point;

        if code_point == AsciiCodePoint::BACKSLASH as i32 {
            i = (i + 2).min(end_index);
            continue;
        }

        if code_point == AsciiCodePoint::OPEN_BRACKET as i32 {
            // `![...]` belongs to image syntax; link tokenizer should skip it.
            if i > block_start_index
                && node_points[i - 1].code_point == AsciiCodePoint::EXCLAMATION_MARK as i32
                && !is_escaped_node_points(node_points, i - 1, block_start_index)
            {
                i += 1;
                continue;
            }

            return Some(DelimiterEntry {
                delimiter: create_delimiter(DelimiterType::Opener, i, i + 1),
                data: None,
            });
        }

        if code_point == AsciiCodePoint::CLOSE_BRACKET as i32
            && i + 1 < end_index
            && node_points[i + 1].code_point == AsciiCodePoint::OPEN_PARENTHESIS as i32
        {
            let destination_start_index =
                eat_optional_whitespaces(node_points, i + 2, block_end_index);
            let destination_end_index =
                eat_link_destination(node_points, destination_start_index, block_end_index);
            if destination_end_index < 0 {
                i += 1;
                continue;
            }
            let destination_end_index = destination_end_index as usize;
            let title_start_index =
                eat_optional_whitespaces(node_points, destination_end_index, block_end_index);
            let title_end_index = eat_link_title(node_points, title_start_index, block_end_index);
            if title_end_index < 0 {
                i += 1;
                continue;
            }
            let title_end_index = title_end_index as usize;
            let absolute_end =
                eat_optional_whitespaces(node_points, title_end_index, block_end_index) + 1;
            if absolute_end > block_end_index
                || node_points[absolute_end - 1].code_point
                    != AsciiCodePoint::CLOSE_PARENTHESIS as i32
            {
                i += 1;
                continue;
            }

            return Some(DelimiterEntry {
                delimiter: create_delimiter(DelimiterType::Closer, i, absolute_end),
                data: Some(LinkDelimiterData {
                    destination_content: (destination_start_index < destination_end_index)
                        .then_some(NodeInterval {
                            start_index: destination_start_index,
                            end_index: destination_end_index,
                        }),
                    title_content: (title_start_index < title_end_index).then_some(NodeInterval {
                        start_index: title_start_index,
                        end_index: title_end_index,
                    }),
                }),
            });
        }

        i += 1;
    }

    None
}

pub(crate) fn create_link_token(
    node_points: &[NodePoint],
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
    data: LinkDelimiterData,
    children_tokens: Vec<InlineToken>,
) -> InlineToken {
    let label = calc_escaped_string_from_node_points(
        node_points,
        opener_delimiter.end_index,
        closer_delimiter.start_index,
        false,
    );

    let token_children = children_tokens.clone();
    InlineToken::new(
        "",
        LINK_TYPE,
        (opener_delimiter.start_index, closer_delimiter.end_index),
    )
    .with_children(token_children)
    .with_data(LinkTokenData {
        destination_content: data.destination_content,
        title_content: data.title_content,
        label,
    })
}

pub(crate) fn check_balanced_brackets_status(
    start_index: usize,
    end_index: usize,
    internal_tokens: &[InlineToken],
    node_points: &[NodePoint],
) -> i8 {
    let mut i = start_index;
    let mut bracket_count = 0i32;

    let update = |idx: usize, count: &mut i32, i_ref: &mut usize| match node_points[idx].code_point
    {
        x if x == AsciiCodePoint::BACKSLASH as i32 => {
            *i_ref += 1;
        }
        x if x == AsciiCodePoint::OPEN_BRACKET as i32 => {
            *count += 1;
        }
        x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
            *count -= 1;
        }
        _ => {}
    };

    for token in internal_tokens {
        if token.start_index < start_index {
            continue;
        }
        if token.end_index > end_index {
            break;
        }

        while i < token.start_index {
            update(i, &mut bracket_count, &mut i);
            if bracket_count < 0 {
                return -1;
            }
            i += 1;
        }

        i = token.end_index;
    }

    while i < end_index {
        update(i, &mut bracket_count, &mut i);
        if bracket_count < 0 {
            return -1;
        }
        i += 1;
    }

    if bracket_count > 0 {
        1
    } else {
        0
    }
}

pub(crate) fn is_escaped_node_points(
    node_points: &[NodePoint],
    index: usize,
    min_index: usize,
) -> bool {
    if index == 0 || index <= min_index {
        return false;
    }

    let mut i = index;
    let mut slash_count = 0usize;
    while i > min_index {
        i -= 1;
        if node_points[i].code_point == AsciiCodePoint::BACKSLASH as i32 {
            slash_count += 1;
            continue;
        }
        break;
    }

    slash_count % 2 == 1
}

fn create_delimiter(
    delimiter_type: DelimiterType,
    start_index: usize,
    end_index: usize,
) -> TokenDelimiter {
    TokenDelimiter {
        delimiter_type,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
    }
}
