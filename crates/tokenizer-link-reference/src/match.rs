use yozora_ast::{ReferenceType, LINK_REFERENCE_TYPE};
use yozora_character::{calc_escaped_string_from_node_points, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{
    eat_link_label, DelimiterType, InlineToken, MatchInlinePhaseApi, TokenDelimiter,
};

use crate::parse::LinkReferenceTokenData;

#[derive(Debug, Clone)]
pub struct LinkReferenceDelimiterBracket {
    pub start_index: usize,
    pub end_index: usize,
    pub label: Option<String>,
    pub identifier: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
    pub brackets: Vec<LinkReferenceDelimiterBracket>,
}

pub(crate) fn find_link_reference_delimiter_entry(
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

        if code_point == AsciiCodePoint::OPEN_BRACKET as i32 {
            let mut brackets: Vec<LinkReferenceDelimiterBracket> = Vec::new();

            let (next_index, label_and_identifier) = eat_link_label(node_points, i, end_index);
            if next_index < 0 {
                return Some(DelimiterEntry {
                    delimiter: create_delimiter(DelimiterType::Opener, i, i + 1),
                    brackets,
                });
            }

            let next_index = next_index as usize;
            if let Some((label, identifier)) = label_and_identifier {
                brackets.push(LinkReferenceDelimiterBracket {
                    start_index: i,
                    end_index: next_index,
                    label: Some(label),
                    identifier: Some(identifier),
                });
            } else {
                // Preceding `[]` is useless.
                i = next_index;
                continue;
            }

            let mut delimiter_type = DelimiterType::Opener;
            let mut delimiter_end = i + 1;
            let mut j = next_index;

            while j < end_index && node_points[j].code_point == AsciiCodePoint::OPEN_BRACKET as i32
            {
                let (next_j, label_and_identifier) = eat_link_label(node_points, j, end_index);

                if next_j < 0 {
                    delimiter_type = DelimiterType::Opener;
                    delimiter_end = j + 1;
                    break;
                }

                let next_j = next_j as usize;
                let mut bracket = LinkReferenceDelimiterBracket {
                    start_index: j,
                    end_index: next_j,
                    label: None,
                    identifier: None,
                };

                delimiter_type = DelimiterType::Full;
                delimiter_end = next_j;

                if let Some((label, identifier)) = label_and_identifier {
                    bracket.label = Some(label);
                    bracket.identifier = Some(identifier);
                    brackets.push(bracket);
                    j = next_j;
                    continue;
                }

                brackets.push(bracket);
                break;
            }

            return Some(DelimiterEntry {
                delimiter: create_delimiter(delimiter_type, i, delimiter_end),
                brackets,
            });
        }

        if code_point == AsciiCodePoint::CLOSE_BRACKET as i32
            && i + 1 < end_index
            && node_points[i + 1].code_point == AsciiCodePoint::OPEN_BRACKET as i32
        {
            let (next_index, label_and_identifier) = eat_link_label(node_points, i + 1, end_index);

            if next_index < 0 {
                return Some(DelimiterEntry {
                    delimiter: create_delimiter(DelimiterType::Opener, i + 1, i + 2),
                    brackets: Vec::new(),
                });
            }

            let next_index = next_index as usize;
            let Some((label, identifier)) = label_and_identifier else {
                // It's `][]`, which is useless here.
                i = next_index;
                continue;
            };

            let mut brackets = vec![LinkReferenceDelimiterBracket {
                start_index: i + 1,
                end_index: next_index,
                label: Some(label),
                identifier: Some(identifier),
            }];

            let mut delimiter_type = DelimiterType::Closer;
            let mut delimiter_end = next_index;
            let mut j = next_index;

            while j < end_index && node_points[j].code_point == AsciiCodePoint::OPEN_BRACKET as i32
            {
                let (next_j, label_and_identifier) = eat_link_label(node_points, j, end_index);
                if next_j < 0 {
                    delimiter_type = DelimiterType::Both;
                    delimiter_end = j + 1;
                    break;
                }

                let next_j = next_j as usize;
                let mut bracket = LinkReferenceDelimiterBracket {
                    start_index: j,
                    end_index: next_j,
                    label: None,
                    identifier: None,
                };

                delimiter_type = DelimiterType::Full;
                delimiter_end = next_j;

                if let Some((label, identifier)) = label_and_identifier {
                    bracket.label = Some(label);
                    bracket.identifier = Some(identifier);
                    brackets.push(bracket);
                    j = next_j;
                    continue;
                }

                brackets.push(bracket);
                break;
            }

            return Some(DelimiterEntry {
                delimiter: create_delimiter(delimiter_type, i, delimiter_end),
                brackets,
            });
        }

        i += 1;
    }

    None
}

pub(crate) fn process_single_delimiter(
    api: &dyn MatchInlinePhaseApi,
    delimiter: &TokenDelimiter,
    brackets: &[LinkReferenceDelimiterBracket],
    node_points: &[NodePoint],
) -> Vec<InlineToken> {
    if brackets.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let mut bracket_index = 0usize;
    let mut last_bracket_index: isize = -1;

    while bracket_index < brackets.len() {
        let mut found_index: Option<usize> = None;
        while bracket_index < brackets.len() {
            let bracket = &brackets[bracket_index];
            if let Some(identifier) = &bracket.identifier {
                if api.has_definition(identifier) {
                    found_index = Some(bracket_index);
                    break;
                }
            }
            bracket_index += 1;
        }

        let Some(current_index) = found_index else {
            break;
        };

        let bracket = &brackets[current_index];
        let (Some(label), Some(identifier)) = (bracket.label.clone(), bracket.identifier.clone())
        else {
            bracket_index += 1;
            continue;
        };

        // Full reference: `[label0][label1]` where the second bracket resolves.
        if (last_bracket_index + 1) < current_index as isize {
            let previous = &brackets[current_index - 1];
            let children_tokens = api.resolve_internal_tokens(
                &[],
                previous.start_index + 1,
                previous.end_index.saturating_sub(1),
            );

            tokens.push(create_reference_token(
                node_points,
                previous.start_index,
                bracket.end_index,
                ReferenceType::Full,
                label,
                identifier,
                previous.start_index + 1,
                previous.end_index.saturating_sub(1),
                children_tokens,
            ));

            last_bracket_index = current_index as isize;
            bracket_index = current_index + 1;
            continue;
        }

        // Shortcut reference: `[label]`.
        if current_index + 1 == brackets.len() {
            let children_tokens = api.resolve_internal_tokens(
                &[],
                bracket.start_index + 1,
                bracket.end_index.saturating_sub(1),
            );

            tokens.push(create_reference_token(
                node_points,
                bracket.start_index,
                bracket.end_index,
                ReferenceType::Shortcut,
                label,
                identifier,
                bracket.start_index + 1,
                bracket.end_index.saturating_sub(1),
                children_tokens,
            ));
            break;
        }

        // Collapsed reference: `[label][]`.
        if current_index + 1 < brackets.len() && brackets[current_index + 1].identifier.is_none() {
            let collapsed_tail = &brackets[current_index + 1];
            let children_tokens = api.resolve_internal_tokens(
                &[],
                bracket.start_index + 1,
                bracket.end_index.saturating_sub(1),
            );

            tokens.push(create_reference_token(
                node_points,
                bracket.start_index,
                collapsed_tail.end_index,
                ReferenceType::Collapsed,
                label,
                identifier,
                bracket.start_index + 1,
                bracket.end_index.saturating_sub(1),
                children_tokens,
            ));
            break;
        }

        bracket_index = current_index + 1;
    }

    if delimiter.start_index == delimiter.end_index {
        return Vec::new();
    }

    tokens
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_reference_token(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    reference_type: ReferenceType,
    label: String,
    identifier: String,
    child_start_index: usize,
    child_end_index: usize,
    children_tokens: Vec<InlineToken>,
) -> InlineToken {
    let child_text = calc_escaped_string_from_node_points(
        node_points,
        child_start_index,
        child_end_index,
        false,
    );

    let token_children = children_tokens.clone();
    InlineToken::new("", LINK_REFERENCE_TYPE, (start_index, end_index))
        .with_children(token_children)
        .with_data(LinkReferenceTokenData {
            identifier,
            label,
            reference_type,
            child_text,
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
