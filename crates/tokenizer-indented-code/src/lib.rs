use yozora_ast::{Code, Node};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
    leading_indent_columns, strip_indent_columns,
};

pub const INDENTED_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-indented-code";

#[derive(Debug, Clone)]
pub struct IndentedCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for IndentedCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: INDENTED_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 4,
            },
        }
    }
}

impl Tokenizer for IndentedCodeTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for IndentedCodeTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        if !is_indented_line(first) {
            return None;
        }

        let mut raw_lines = Vec::new();
        let mut consumed_lines = 0usize;

        for line in lines {
            if let Some(stripped) = strip_indented_prefix(line) {
                raw_lines.push(stripped);
                consumed_lines += 1;
                continue;
            }

            if line.trim().is_empty() {
                raw_lines.push(String::new());
                consumed_lines += 1;
                continue;
            }

            break;
        }

        let start = raw_lines
            .iter()
            .position(|line| !line.is_empty())
            .unwrap_or(0);
        let end = raw_lines
            .iter()
            .rposition(|line| !line.is_empty())
            .map(|index| index + 1)
            .unwrap_or(0);
        if start >= end {
            return None;
        }

        let content = format!("{}\n", raw_lines[start..end].join("\n"));

        Some(BlockTokenizeResult {
            node: Node::Code(Code {
                position: None,
                value: content,
                lang: None,
                meta: None,
            }),
            consumed_lines,
        })
    }
}

fn is_indented_line(line: &str) -> bool {
    leading_indent_columns(line) >= 4
}

fn strip_indented_prefix(line: &str) -> Option<String> {
    strip_indent_columns(line, 4)
}
