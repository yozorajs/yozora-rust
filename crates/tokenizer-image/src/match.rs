use yozora_ast::IMAGE_TYPE;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{eat_optional_whitespaces, DelimiterType, InlineToken, NodeInterval};
use yozora_tokenizer_link::{eat_link_destination, eat_link_title};

use crate::types::{ImageDelimiter, ImageTokenData};

pub(crate) fn find_image_delimiter_entry(
    node_points: &[NodePoint],
    block_end_index: usize,
    start_index: usize,
    end_index: usize,
) -> Option<ImageDelimiter> {
    let mut i = start_index;

    while i < end_index {
        let code_point = node_points[i].code_point;

        if code_point == AsciiCodePoint::BACKSLASH as i32 {
            i = (i + 2).min(end_index);
            continue;
        }

        if code_point == AsciiCodePoint::EXCLAMATION_MARK as i32 {
            if i + 1 < end_index
                && node_points[i + 1].code_point == AsciiCodePoint::OPEN_BRACKET as i32
            {
                return Some(create_delimiter(
                    DelimiterType::Opener,
                    i,
                    i + 2,
                    None,
                    None,
                ));
            }

            i += 1;
            continue;
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

            let delimiter_start_index = i;
            let delimiter_end_index =
                eat_optional_whitespaces(node_points, title_end_index, block_end_index) + 1;
            if delimiter_end_index > block_end_index
                || node_points[delimiter_end_index - 1].code_point
                    != AsciiCodePoint::CLOSE_PARENTHESIS as i32
            {
                i += 1;
                continue;
            }

            return Some(create_delimiter(
                DelimiterType::Closer,
                delimiter_start_index,
                delimiter_end_index,
                (destination_start_index < destination_end_index).then_some(NodeInterval {
                    start_index: destination_start_index,
                    end_index: destination_end_index,
                }),
                (title_start_index < title_end_index).then_some(NodeInterval {
                    start_index: title_start_index,
                    end_index: title_end_index,
                }),
            ));
        }

        i += 1;
    }

    None
}

pub(crate) fn create_image_token(
    opener_delimiter: &ImageDelimiter,
    closer_delimiter: &ImageDelimiter,
    children_tokens: Vec<InlineToken>,
) -> InlineToken {
    InlineToken::new(
        "",
        IMAGE_TYPE,
        (opener_delimiter.start_index, closer_delimiter.end_index),
    )
    .with_children(children_tokens)
    .with_data(ImageTokenData {
        destination_content: closer_delimiter.destination_content,
        title_content: closer_delimiter.title_content,
    })
}

fn create_delimiter(
    delimiter_type: DelimiterType,
    start_index: usize,
    end_index: usize,
    destination_content: Option<NodeInterval>,
    title_content: Option<NodeInterval>,
) -> ImageDelimiter {
    ImageDelimiter {
        delimiter_type,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
        destination_content,
        title_content,
    }
}
