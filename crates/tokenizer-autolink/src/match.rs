use yozora_ast::LINK_TYPE;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{DelimiterType, InlineToken, MatchInlinePhaseApi};

use crate::types::{
    AutolinkContentHelper, AutolinkContentType, AutolinkDelimiter, AutolinkTokenData,
};
use crate::util::{eat_absolute_uri, eat_email_address};

const HELPERS: &[AutolinkContentHelper] = &[
    AutolinkContentHelper {
        content_type: AutolinkContentType::Uri,
        eat: eat_absolute_uri,
    },
    AutolinkContentHelper {
        content_type: AutolinkContentType::Email,
        eat: eat_email_address,
    },
];

pub(crate) fn find_delimiter_entry(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<AutolinkDelimiter> {
    let mut i = start_index;
    while i < end_index {
        if node_points[i].code_point != AsciiCodePoint::OPEN_ANGLE as i32 {
            i += 1;
            continue;
        }

        let mut next_index = end_index;
        let mut content_type: Option<AutolinkContentType> = None;
        if i + 1 < end_index {
            for helper in HELPERS {
                let eat_result = (helper.eat)(node_points, i + 1, end_index);
                next_index = next_index.min(eat_result.next_index);
                if eat_result.valid {
                    content_type = Some(helper.content_type);
                    next_index = eat_result.next_index;
                    break;
                }
            }
        }

        let Some(content_type) = content_type else {
            let skip_to = std::cmp::max(i + 1, next_index.saturating_sub(1));
            i = skip_to;
            continue;
        };

        if next_index < end_index
            && node_points[next_index].code_point == AsciiCodePoint::CLOSE_ANGLE as i32
        {
            return Some(AutolinkDelimiter {
                delimiter_type: DelimiterType::Full,
                start_index: i,
                end_index: next_index + 1,
                thickness: next_index + 1 - i,
                original_thickness: next_index + 1 - i,
                content_type,
            });
        }

        let skip_to = std::cmp::max(i + 1, next_index.saturating_sub(1));
        i = skip_to;
    }

    None
}

pub(crate) fn process_single_delimiter(
    _api: &dyn MatchInlinePhaseApi,
    delimiter: &AutolinkDelimiter,
) -> Vec<InlineToken> {
    vec![
        InlineToken::new("", LINK_TYPE, (delimiter.start_index, delimiter.end_index)).with_data(
            AutolinkTokenData {
                content_type: delimiter.content_type,
            },
        ),
    ]
}
