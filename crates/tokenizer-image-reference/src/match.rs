use yozora_ast::{ReferenceType, IMAGE_REFERENCE_TYPE};
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    eat_link_label, DelimiterType, InlineToken, MatchInlinePhaseApi, TokenDelimiter,
};

use crate::parse::ImageReferenceTokenData;

#[derive(Debug, Clone)]
pub(crate) struct ImageReferenceDelimiterBracket {
    pub end_index: usize,
    pub label: Option<String>,
    pub identifier: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
    pub brackets: Vec<ImageReferenceDelimiterBracket>,
}

pub(crate) fn find_image_reference_delimiter_entry(
    node_points: &[NodePoint],
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

        if code_point == AsciiCodePoint::EXCLAMATION_MARK as i32 {
            if i + 1 < end_index
                && node_points[i + 1].code_point == AsciiCodePoint::OPEN_BRACKET as i32
            {
                return Some(DelimiterEntry {
                    delimiter: create_delimiter(DelimiterType::Opener, i, i + 2),
                    brackets: Vec::new(),
                });
            }

            i += 1;
            continue;
        }

        if code_point == AsciiCodePoint::CLOSE_BRACKET as i32 {
            let mut delimiter = DelimiterEntry {
                delimiter: create_delimiter(DelimiterType::Closer, i, i + 1),
                brackets: Vec::new(),
            };

            if i + 1 >= end_index
                || node_points[i + 1].code_point != AsciiCodePoint::OPEN_BRACKET as i32
            {
                return Some(delimiter);
            }

            let (next_index, label_and_identifier) = eat_link_label(node_points, i + 1, end_index);
            if next_index < 0 {
                return Some(delimiter);
            }

            let next_index = next_index as usize;
            delimiter.delimiter = create_delimiter(DelimiterType::Closer, i, next_index);

            let mut bracket = ImageReferenceDelimiterBracket {
                end_index: next_index,
                label: None,
                identifier: None,
            };

            if let Some((label, identifier)) = label_and_identifier {
                bracket.label = Some(label);
                bracket.identifier = Some(identifier);
            }

            delimiter.brackets.push(bracket);
            return Some(delimiter);
        }

        i += 1;
    }

    None
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

pub(crate) fn process_delimiter_pair(
    api: &dyn MatchInlinePhaseApi,
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
    closer_brackets: &[ImageReferenceDelimiterBracket],
    internal_tokens: &[InlineToken],
    node_points: &[NodePoint],
) -> Vec<InlineToken> {
    let bracket = closer_brackets.first();
    if let Some(bracket) = bracket {
        if let (Some(label), Some(identifier)) = (bracket.label.clone(), bracket.identifier.clone())
        {
            if api.has_definition(&identifier) {
                let children_tokens = api.resolve_internal_tokens(
                    internal_tokens,
                    opener_delimiter.end_index,
                    closer_delimiter.start_index,
                );
                return vec![create_reference_token(
                    opener_delimiter.start_index,
                    bracket.end_index,
                    ReferenceType::Full,
                    label,
                    identifier,
                    children_tokens,
                )];
            }

            return internal_tokens.to_vec();
        }
    }

    let (next_index, label_and_identifier) = eat_link_label(
        node_points,
        opener_delimiter.end_index.saturating_sub(1),
        closer_delimiter.start_index + 1,
    );

    if next_index >= 0 && next_index as usize == closer_delimiter.start_index + 1 {
        if let Some((label, identifier)) = label_and_identifier {
            if api.has_definition(&identifier) {
                let reference_type = if bracket.is_none() {
                    ReferenceType::Shortcut
                } else {
                    ReferenceType::Collapsed
                };

                let children_tokens = api.resolve_internal_tokens(
                    internal_tokens,
                    opener_delimiter.end_index,
                    closer_delimiter.start_index,
                );

                return vec![create_reference_token(
                    opener_delimiter.start_index,
                    closer_delimiter.end_index,
                    reference_type,
                    label,
                    identifier,
                    children_tokens,
                )];
            }
        }
    }

    internal_tokens.to_vec()
}

fn create_reference_token(
    start_index: usize,
    end_index: usize,
    reference_type: ReferenceType,
    label: String,
    identifier: String,
    children_tokens: Vec<InlineToken>,
) -> InlineToken {
    InlineToken::new("", IMAGE_REFERENCE_TYPE, (start_index, end_index)).with_data(
        ImageReferenceTokenData {
            identifier,
            label,
            reference_type,
            children_tokens,
        },
    )
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
