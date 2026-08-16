use yozora_ast::HTML_TYPE;
use yozora_character::{is_ascii_upper_letter, AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{eat_optional_whitespaces, InlineToken};

use crate::types::HtmlInlineDelimiter;
use crate::util::{
    eat_html_inline_cdata_delimiter, eat_html_inline_closing_delimiter,
    eat_html_inline_comment_delimiter, eat_html_inline_declaration_delimiter,
    eat_html_inline_instruction_delimiter, eat_html_inline_token_open_delimiter,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct HtmlInlineCloserCache {
    cdata: isize,
    declaration: isize,
    end_index: isize,
    instruction: isize,
}

impl Default for HtmlInlineCloserCache {
    fn default() -> Self {
        Self {
            cdata: -1,
            declaration: -1,
            end_index: -1,
            instruction: -1,
        }
    }
}

pub(crate) fn find_html_inline_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    closer_cache: &mut HtmlInlineCloserCache,
) -> Option<HtmlInlineDelimiter> {
    let mut i = start_index;
    while i < end_index {
        i = eat_optional_whitespaces(node_points, i, end_index);
        if i >= end_index {
            break;
        }

        let code_point = node_points[i].code_point;
        if code_point == AsciiCodePoint::BACKSLASH as i32 {
            i = (i + 2).min(end_index);
            continue;
        }

        if code_point == AsciiCodePoint::OPEN_ANGLE as i32 {
            if let Some(delimiter) = try_to_eat_delimiter(node_points, closer_cache, i, end_index) {
                return Some(delimiter);
            }
        }

        i += 1;
    }

    None
}

pub(crate) fn process_single_delimiter(delimiter: &HtmlInlineDelimiter) -> Vec<InlineToken> {
    let core_delimiter = delimiter.delimiter();
    vec![InlineToken::new(
        "",
        HTML_TYPE,
        (core_delimiter.start_index, core_delimiter.end_index),
    )
    .with_data(delimiter.token_data())]
}

fn try_to_eat_delimiter(
    node_points: &[NodePoint],
    closer_cache: &mut HtmlInlineCloserCache,
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineDelimiter> {
    if let Some(delimiter) =
        eat_html_inline_token_open_delimiter(node_points, start_index, end_index)
    {
        return Some(HtmlInlineDelimiter::Open(delimiter));
    }

    if let Some(delimiter) = eat_html_inline_closing_delimiter(node_points, start_index, end_index)
    {
        return Some(HtmlInlineDelimiter::Closing(delimiter));
    }

    if let Some(delimiter) = eat_html_inline_comment_delimiter(node_points, start_index, end_index)
    {
        return Some(HtmlInlineDelimiter::Comment(delimiter));
    }

    let second_code_point = node_points
        .get(start_index + 1)
        .map(|point| point.code_point);
    if second_code_point == Some(AsciiCodePoint::QUESTION_MARK as i32)
        && may_have_closer(
            closer_cache,
            closer_cache.instruction,
            start_index + 2,
            end_index,
        )
    {
        if let Some(delimiter) =
            eat_html_inline_instruction_delimiter(node_points, start_index, end_index)
        {
            return Some(HtmlInlineDelimiter::Instruction(delimiter));
        }
        update_closer_cache(node_points, closer_cache, start_index + 2, end_index);
    }

    let third_code_point = node_points
        .get(start_index + 2)
        .map(|point| point.code_point);
    if second_code_point == Some(AsciiCodePoint::EXCLAMATION_MARK as i32)
        && third_code_point.is_some_and(is_ascii_upper_letter)
        && may_have_closer(
            closer_cache,
            closer_cache.declaration,
            start_index + 3,
            end_index,
        )
    {
        if let Some(delimiter) =
            eat_html_inline_declaration_delimiter(node_points, start_index, end_index)
        {
            return Some(HtmlInlineDelimiter::Declaration(delimiter));
        }
        update_closer_cache(node_points, closer_cache, start_index + 3, end_index);
    }

    if second_code_point == Some(AsciiCodePoint::EXCLAMATION_MARK as i32)
        && third_code_point == Some(AsciiCodePoint::OPEN_BRACKET as i32)
        && may_have_closer(closer_cache, closer_cache.cdata, start_index + 3, end_index)
    {
        if let Some(delimiter) =
            eat_html_inline_cdata_delimiter(node_points, start_index, end_index)
        {
            return Some(HtmlInlineDelimiter::Cdata(delimiter));
        }
        update_closer_cache(node_points, closer_cache, start_index + 3, end_index);
    }

    None
}

fn may_have_closer(
    cache: &HtmlInlineCloserCache,
    closer_start_index: isize,
    start_index: usize,
    end_index: usize,
) -> bool {
    end_index as isize != cache.end_index || closer_start_index >= start_index as isize
}

fn update_closer_cache(
    node_points: &[NodePoint],
    cache: &mut HtmlInlineCloserCache,
    start_index: usize,
    end_index: usize,
) {
    if end_index as isize == cache.end_index {
        return;
    }

    cache.cdata = -1;
    cache.declaration = -1;
    cache.instruction = -1;
    cache.end_index = end_index as isize;

    let mut i = end_index;
    while i > start_index {
        i -= 1;
        if node_points[i].code_point != AsciiCodePoint::CLOSE_ANGLE as i32 {
            continue;
        }

        if cache.declaration < 0 {
            cache.declaration = i as isize;
        }
        if cache.instruction < 0
            && i > 0
            && node_points[i - 1].code_point == AsciiCodePoint::QUESTION_MARK as i32
        {
            cache.instruction = (i - 1) as isize;
        }
        if cache.cdata < 0
            && i > 1
            && node_points[i - 1].code_point == AsciiCodePoint::CLOSE_BRACKET as i32
            && node_points[i - 2].code_point == AsciiCodePoint::CLOSE_BRACKET as i32
        {
            cache.cdata = (i - 2) as isize;
        }
        if cache.cdata >= 0 && cache.instruction >= 0 {
            break;
        }
    }
}
