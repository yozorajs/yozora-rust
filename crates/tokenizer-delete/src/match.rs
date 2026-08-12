use yozora_ast::DELETE_TYPE;
use yozora_character::{is_whitespace_character, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{DelimiterType, InlineToken, TokenDelimiter};

use crate::parse::DeleteTokenData;

pub(crate) fn find_delete_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<TokenDelimiter> {
    if start_index >= end_index || end_index > node_points.len() {
        return None;
    }

    let mut i = start_index;
    while i < end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::BACKSLASH as i32 {
            i += 2;
            continue;
        }

        if c != AsciiCodePoint::TILDE as i32 {
            i += 1;
            continue;
        }

        let start = i;
        i += 1;
        while i < end_index && node_points[i].code_point == c {
            i += 1;
        }

        let end = i;
        let thickness = end - start;
        if thickness != 1 && thickness != 2 {
            continue;
        }

        let mut delimiter_type = DelimiterType::Both;

        let preceding = if start == start_index {
            None
        } else {
            node_points.get(start - 1)
        };
        if preceding.is_some_and(|p| is_whitespace_character(p.code_point)) {
            delimiter_type = DelimiterType::Opener;
        }

        let following = if end == end_index {
            None
        } else {
            node_points.get(end)
        };
        if following.is_some_and(|p| is_whitespace_character(p.code_point)) {
            if delimiter_type != DelimiterType::Both {
                continue;
            }
            delimiter_type = DelimiterType::Closer;
        }

        return Some(TokenDelimiter {
            delimiter_type,
            start_index: start,
            end_index: end,
            thickness,
            original_thickness: thickness,
        });
    }

    None
}

pub(crate) fn create_delete_token(
    opener_delimiter: &TokenDelimiter,
    closer_delimiter: &TokenDelimiter,
    children: Vec<InlineToken>,
) -> InlineToken {
    let token_children = children.clone();
    InlineToken::new(
        "",
        DELETE_TYPE,
        (opener_delimiter.start_index, closer_delimiter.end_index),
    )
    .with_children(token_children)
    .with_data(DeleteTokenData)
}
