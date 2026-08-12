use yozora_ast::HTML_TYPE;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{InlineToken, TokenDelimiter};

use crate::util::{
    eat_html_inline_cdata_delimiter, eat_html_inline_closing_delimiter,
    eat_html_inline_comment_delimiter, eat_html_inline_declaration_delimiter,
    eat_html_inline_instruction_delimiter, eat_html_inline_token_open_delimiter,
    HtmlInlineCDataDelimiter, HtmlInlineCDataTokenData, HtmlInlineClosingDelimiter,
    HtmlInlineClosingTokenData, HtmlInlineCommentDelimiter, HtmlInlineCommentTokenData,
    HtmlInlineDeclarationDelimiter, HtmlInlineDeclarationTokenData, HtmlInlineInstructionDelimiter,
    HtmlInlineInstructionTokenData, HtmlInlineOpenDelimiter, HtmlInlineOpenTokenData,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlInlineTokenData {
    Open(HtmlInlineOpenTokenData),
    Closing(HtmlInlineClosingTokenData),
    Comment(HtmlInlineCommentTokenData),
    Instruction(HtmlInlineInstructionTokenData),
    Declaration(HtmlInlineDeclarationTokenData),
    Cdata(HtmlInlineCDataTokenData),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HtmlInlineDelimiter {
    Open(HtmlInlineOpenDelimiter),
    Closing(HtmlInlineClosingDelimiter),
    Comment(HtmlInlineCommentDelimiter),
    Instruction(HtmlInlineInstructionDelimiter),
    Declaration(HtmlInlineDeclarationDelimiter),
    Cdata(HtmlInlineCDataDelimiter),
}

impl HtmlInlineDelimiter {
    pub(crate) fn delimiter(&self) -> &TokenDelimiter {
        match self {
            Self::Open(value) => &value.delimiter,
            Self::Closing(value) => &value.delimiter,
            Self::Comment(value) => &value.delimiter,
            Self::Instruction(value) => &value.delimiter,
            Self::Declaration(value) => &value.delimiter,
            Self::Cdata(value) => &value.delimiter,
        }
    }

    pub(crate) fn token_data(&self) -> HtmlInlineTokenData {
        match self {
            Self::Open(value) => HtmlInlineTokenData::Open(HtmlInlineOpenTokenData {
                html_type: value.html_type,
                tag_name: value.tag_name,
                attributes: value.attributes.clone(),
                self_closed: value.self_closed,
            }),
            Self::Closing(value) => HtmlInlineTokenData::Closing(HtmlInlineClosingTokenData {
                html_type: value.html_type,
                tag_name: value.tag_name,
            }),
            Self::Comment(value) => HtmlInlineTokenData::Comment(HtmlInlineCommentTokenData {
                html_type: value.html_type,
            }),
            Self::Instruction(value) => {
                HtmlInlineTokenData::Instruction(HtmlInlineInstructionTokenData {
                    html_type: value.html_type,
                })
            }
            Self::Declaration(value) => {
                HtmlInlineTokenData::Declaration(HtmlInlineDeclarationTokenData {
                    html_type: value.html_type,
                    tag_name: value.tag_name,
                })
            }
            Self::Cdata(value) => HtmlInlineTokenData::Cdata(HtmlInlineCDataTokenData {
                html_type: value.html_type,
            }),
        }
    }
}

pub(crate) fn find_html_inline_delimiter(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineDelimiter> {
    let mut index = start_index;
    while index < end_index {
        if node_points[index].code_point == AsciiCodePoint::OPEN_ANGLE as i32
            && !is_escaped(node_points, index)
        {
            if let Some(delimiter) = find_html_inline_delimiter_at(node_points, index, end_index) {
                return Some(delimiter);
            }
        }
        index += 1;
    }
    None
}

fn is_escaped(node_points: &[NodePoint], index: usize) -> bool {
    let mut backslash_count = 0usize;
    let mut cursor = index;
    while cursor > 0 && node_points[cursor - 1].code_point == AsciiCodePoint::BACKSLASH as i32 {
        backslash_count += 1;
        cursor -= 1;
    }
    backslash_count % 2 == 1
}

fn find_html_inline_delimiter_at(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<HtmlInlineDelimiter> {
    if let Some(value) = eat_html_inline_comment_delimiter(node_points, start_index, end_index) {
        return Some(HtmlInlineDelimiter::Comment(value));
    }
    if let Some(value) = eat_html_inline_instruction_delimiter(node_points, start_index, end_index)
    {
        return Some(HtmlInlineDelimiter::Instruction(value));
    }
    if let Some(value) = eat_html_inline_cdata_delimiter(node_points, start_index, end_index) {
        return Some(HtmlInlineDelimiter::Cdata(value));
    }
    if let Some(value) = eat_html_inline_declaration_delimiter(node_points, start_index, end_index)
    {
        return Some(HtmlInlineDelimiter::Declaration(value));
    }
    if let Some(value) = eat_html_inline_closing_delimiter(node_points, start_index, end_index) {
        return Some(HtmlInlineDelimiter::Closing(value));
    }
    eat_html_inline_token_open_delimiter(node_points, start_index, end_index)
        .map(HtmlInlineDelimiter::Open)
}

pub(crate) fn process_single_delimiter(
    node_points: &[NodePoint],
    delimiter: &TokenDelimiter,
) -> Vec<InlineToken> {
    let Some(specific) =
        find_html_inline_delimiter_at(node_points, delimiter.start_index, delimiter.end_index)
    else {
        return Vec::new();
    };
    if specific.delimiter().end_index != delimiter.end_index {
        return Vec::new();
    }
    vec![
        InlineToken::new("", HTML_TYPE, (delimiter.start_index, delimiter.end_index))
            .with_data(specific.token_data()),
    ]
}
