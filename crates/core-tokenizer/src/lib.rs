pub mod engine;
pub mod phase;

use yozora_ast::{Node, Position};

pub use phase::*;

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

pub trait Tokenizer {
    fn meta(&self) -> &TokenizerMeta;
}

pub trait BlockTokenizer: Tokenizer {
    fn can_interrupt_paragraph(&self) -> bool {
        true
    }

    fn can_interrupt_paragraph_with_lines(&self, _lines: &[&str]) -> bool {
        self.can_interrupt_paragraph()
    }

    fn tokenize_block(&self, _input: &str, _position: Option<Position>) -> Option<Node> {
        None
    }

    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        position: Option<Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        let node = self.tokenize_block(first, position)?;
        Some(BlockTokenizeResult {
            node,
            consumed_lines: 1,
        })
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        position: Option<Position>,
        _api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        self.tokenize_block_lines(lines, position)
    }
}

pub trait InlineTokenizer: Tokenizer {
    fn tokenize_inline(&self, _input: &str, _position: Option<Position>) -> Option<Vec<Node>> {
        None
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        position: Option<Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, position)
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        position: Option<Position>,
        match_api: &dyn MatchInlinePhaseApi,
        _parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline_with_api(input, position, match_api)
    }
}

pub trait BlockFallbackTokenizer: BlockTokenizer {
    fn build_block(&self, inline_children: Vec<Node>, position: Option<Position>) -> Node;

    fn build_block_with_api(
        &self,
        inline_children: Vec<Node>,
        position: Option<Position>,
        _api: &dyn ParseBlockPhaseApi,
    ) -> Node {
        self.build_block(inline_children, position)
    }
}

pub trait InlineFallbackTokenizer: InlineTokenizer {
    fn build_inline(&self, value: &str, position: Option<Position>) -> Node;

    fn find_and_handle_delimiter(
        &self,
        source: &str,
        start_index: usize,
        end_index: usize,
        position: Option<Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Node {
        self.build_inline(&source[start_index..end_index], position)
    }
}

pub enum AnyTokenizer {
    Block(Box<dyn BlockTokenizer>),
    Inline(Box<dyn InlineTokenizer>),
}

impl AnyTokenizer {
    pub fn kind(&self) -> TokenizerKind {
        match self {
            Self::Block(_) => TokenizerKind::Block,
            Self::Inline(_) => TokenizerKind::Inline,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Block(t) => &t.meta().name,
            Self::Inline(t) => &t.meta().name,
        }
    }

    pub fn priority(&self) -> i32 {
        match self {
            Self::Block(t) => t.meta().priority,
            Self::Inline(t) => t.meta().priority,
        }
    }
}

pub enum AnyFallbackTokenizer {
    Block(Box<dyn BlockFallbackTokenizer>),
    Inline(Box<dyn InlineFallbackTokenizer>),
}

pub fn leading_indent_columns(line: &str) -> usize {
    split_indent_prefix(line, 0).0
}

pub fn split_indent_prefix(line: &str, base_column: usize) -> (usize, usize) {
    let mut column = base_column;
    let mut consumed_bytes = 0usize;

    for (idx, ch) in line.char_indices() {
        let width = match ch {
            ' ' => 1,
            '\t' => 4 - (column % 4),
            _ => break,
        };

        column += width;
        consumed_bytes = idx + ch.len_utf8();
    }

    (column.saturating_sub(base_column), consumed_bytes)
}

pub fn strip_indent_columns(line: &str, columns: usize) -> Option<String> {
    strip_indent_columns_with_base(line, columns, 0)
}

pub fn strip_indent_columns_with_base(
    line: &str,
    columns: usize,
    base_column: usize,
) -> Option<String> {
    if columns == 0 {
        return Some(line.to_string());
    }

    let mut column = base_column;
    let mut content_start = 0usize;

    for (idx, ch) in line.char_indices() {
        let width = match ch {
            ' ' => 1,
            '\t' => 4 - (column % 4),
            _ => {
                content_start = idx;
                break;
            }
        };

        column += width;
        content_start = idx + ch.len_utf8();
    }

    let total_indent = column.saturating_sub(base_column);
    if total_indent < columns {
        return None;
    }

    let remaining_indent = total_indent - columns;
    Some(format!(
        "{}{}",
        " ".repeat(remaining_indent),
        &line[content_start..]
    ))
}

impl AnyFallbackTokenizer {
    pub fn kind(&self) -> TokenizerKind {
        match self {
            Self::Block(_) => TokenizerKind::Block,
            Self::Inline(_) => TokenizerKind::Inline,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Block(t) => &t.meta().name,
            Self::Inline(t) => &t.meta().name,
        }
    }
}
