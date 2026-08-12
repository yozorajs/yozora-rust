pub mod constant;
pub mod tokenizers;
pub mod types;
pub mod util;

use std::fmt::{Display, Formatter};
use yozora_ast::Node;

pub use constant::{DelimiterType, TokenizerPriority, TokenizerType};
pub use tokenizers::{gen_find_delimiter, BaseBlockTokenizer, BaseInlineTokenizer};
pub use types::match_block::{
    EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatLazyContinuationTextResult,
    EatOpenerResult, MatchBlockHook, MatchBlockPhaseApi, OnCloseResult, RemainingSibling,
};
pub use types::match_inline::{
    FindDelimiterGenerator, IsDelimiterPairResult, MatchInlineFallbackPhaseApi, MatchInlineHook,
    MatchInlinePhaseApi, ProcessDelimiterPairResult,
};
pub use types::parse_block::{
    ParseBlockHook, ParseBlockPhaseApi, ParseBlockTask, ParseBlockTaskStep,
};
pub use types::parse_inline::{ParseInlineHook, ParseInlinePhaseApi};
pub use types::phrasing_content::PhrasingContentLine;
pub use types::token::{BlockToken, BlockTokenChildren, InlineToken, TokenData, TokenDelimiter};
pub use types::tokenizer::{BlockTokenizer, InlineFallbackTokenizer, InlineTokenizer, Tokenizer};
pub use types::util::{NodeInterval, ResultOfOptionalEater, ResultOfRequiredEater};
pub use util::phrasing_content::{
    calc_position_from_phrasing_content_lines, merge_and_strip_content_lines,
    merge_content_lines_faithfully,
};
pub use util::point::{calc_end_point, calc_start_point};
pub use util::uri::{
    check_balanced_brackets_status, contains_link_token, eat_link_label, encode_link_destination,
    is_link_token, is_valid_link_text, resolve_label_to_identifier,
    resolve_link_label_and_identifier,
};
pub use util::whitespace::{
    calc_indent_width, eat_indentation, eat_optional_blank_lines, eat_optional_characters,
    eat_optional_whitespaces, eat_optional_whitespaces_reverse, is_blank_range,
    leading_indent_columns, split_indent_prefix, strip_indent_columns,
    strip_indent_columns_with_base, trim_blank_lines,
};

#[derive(Debug, Clone)]
pub struct BlockTokenizeResult {
    pub node: Node,
    pub consumed_lines: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerKind {
    Block,
    Inline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizerMeta {
    pub name: String,
    pub kind: TokenizerKind,
    pub priority: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenizerOptions {
    pub name: Option<String>,
    pub priority: Option<i32>,
}

pub enum AnyTokenizer {
    Block(Box<dyn BlockTokenizer>),
    Inline(Box<dyn InlineTokenizer>),
}

impl AnyTokenizer {
    pub fn r#type(&self) -> TokenizerType {
        match self {
            Self::Block(t) => t.r#type(),
            Self::Inline(t) => t.r#type(),
        }
    }

    pub fn kind(&self) -> TokenizerKind {
        match self {
            Self::Block(_) => TokenizerKind::Block,
            Self::Inline(_) => TokenizerKind::Inline,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Block(t) => t.name(),
            Self::Inline(t) => t.name(),
        }
    }

    pub fn priority(&self) -> i32 {
        match self {
            Self::Block(t) => t.priority(),
            Self::Inline(t) => t.priority(),
        }
    }
}

impl Display for AnyTokenizer {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

pub enum AnyFallbackTokenizer {
    Block(Box<dyn BlockTokenizer>),
    Inline(Box<dyn InlineFallbackTokenizer>),
}

impl AnyFallbackTokenizer {
    pub fn r#type(&self) -> TokenizerType {
        match self {
            Self::Block(t) => t.r#type(),
            Self::Inline(t) => t.r#type(),
        }
    }

    pub fn kind(&self) -> TokenizerKind {
        match self {
            Self::Block(_) => TokenizerKind::Block,
            Self::Inline(_) => TokenizerKind::Inline,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Block(t) => t.name(),
            Self::Inline(t) => t.name(),
        }
    }
}

impl Display for AnyFallbackTokenizer {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}
