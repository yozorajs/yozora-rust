use yozora_ast::FOOTNOTE_REFERENCE_TYPE;
use yozora_character::AsciiCodePoint;
use yozora_core_tokenizer::{
    resolve_link_label_and_identifier, DelimiterType, InlineToken, MatchInlinePhaseApi,
    TokenDelimiter,
};
use yozora_tokenizer_footnote_definition::eat_footnote_label;

use crate::parse::FootnoteReferenceTokenData;

#[derive(Debug, Clone)]
pub(crate) struct DelimiterEntry {
    pub delimiter: TokenDelimiter,
}

pub(crate) fn find_delimiter_entry(
    api: &dyn MatchInlinePhaseApi,
    start_index: usize,
    end_index: usize,
) -> Option<DelimiterEntry> {
    let node_points = api.getNodePoints();
    let mut i = start_index;

    while i < end_index {
        let code_point = node_points[i].code_point;
        match code_point {
            x if x == AsciiCodePoint::BACKSLASH as i32 => {
                i = (i + 2).min(end_index);
                continue;
            }
            x if x == AsciiCodePoint::OPEN_BRACKET as i32 => {
                let next_index = eat_footnote_label(node_points, i, end_index);
                if next_index >= 0 {
                    return Some(DelimiterEntry {
                        delimiter: create_delimiter(DelimiterType::Full, i, next_index as usize),
                    });
                }
            }
            _ => {}
        }

        i += 1;
    }

    None
}

pub(crate) fn process_single_delimiter(
    api: &dyn MatchInlinePhaseApi,
    delimiter: &TokenDelimiter,
) -> Vec<InlineToken> {
    if delimiter.end_index <= delimiter.start_index + 2 {
        return Vec::new();
    }

    let node_points = api.getNodePoints();
    let Some((label, identifier)) = resolve_link_label_and_identifier(
        node_points,
        delimiter.start_index + 2,
        delimiter.end_index - 1,
    ) else {
        return Vec::new();
    };

    if !api.hasFootnoteDefinition(&identifier) {
        return Vec::new();
    }

    let token = InlineToken::new(
        "",
        FOOTNOTE_REFERENCE_TYPE,
        (delimiter.start_index, delimiter.end_index),
    )
    .with_data(FootnoteReferenceTokenData { identifier, label });

    vec![token]
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
