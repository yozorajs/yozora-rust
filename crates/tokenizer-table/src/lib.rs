use yozora_ast::{AlignType, Node, Table, TableCell, TableColumn, TableRow, Text};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
};

pub const TABLE_TOKENIZER_NAME: &str = "@yozora/tokenizer-table";

#[derive(Debug, Clone)]
pub struct TableTokenizer {
    meta: TokenizerMeta,
}

impl Default for TableTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: TABLE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 5,
            },
        }
    }
}

impl Tokenizer for TableTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for TableTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        if lines.len() < 2 {
            return None;
        }

        if !lines[0].contains('|') && !lines[1].contains('|') {
            return None;
        }

        let header_cells = split_table_cells(lines[0]);
        if header_cells.is_empty() {
            return None;
        }

        let alignments = parse_delimiter_row(lines[1])?;
        if alignments.is_empty() {
            return None;
        }

        if header_cells.len() != alignments.len() {
            return None;
        }

        let column_count = alignments.len();
        let mut rows = vec![build_row_node(&header_cells, column_count)];
        let mut consumed_lines = 2usize;

        for line in lines.iter().skip(2) {
            if line.trim().is_empty() {
                break;
            }
            if starts_blockquote(line) {
                break;
            }
            let cells = split_table_cells(line);
            rows.push(build_row_node(&cells, column_count));
            consumed_lines += 1;
        }

        let columns = alignments
            .into_iter()
            .map(|align| TableColumn { align })
            .collect();

        Some(BlockTokenizeResult {
            node: Node::Table(Table {
                position: None,
                columns,
                children: rows,
            }),
            consumed_lines,
        })
    }
}

fn parse_delimiter_row(line: &str) -> Option<Vec<Option<AlignType>>> {
    let cells = split_table_cells(line);
    if cells.is_empty() {
        return None;
    }

    let mut alignments = Vec::new();
    for cell in cells {
        let trimmed = cell.trim();
        if trimmed.is_empty() {
            return None;
        }

        let align = if trimmed.starts_with(':') && trimmed.ends_with(':') && trimmed.len() >= 3 {
            Some(AlignType::Center)
        } else if trimmed.starts_with(':') && trimmed.len() >= 2 {
            Some(AlignType::Left)
        } else if trimmed.ends_with(':') && trimmed.len() >= 2 {
            Some(AlignType::Right)
        } else {
            None
        };

        let mut bar = trimmed;
        if let Some(rest) = bar.strip_prefix(':') {
            bar = rest;
        }
        if let Some(rest) = bar.strip_suffix(':') {
            bar = rest;
        }
        if bar.is_empty() || !bar.chars().all(|ch| ch == '-') {
            return None;
        }

        alignments.push(align);
    }

    Some(alignments)
}

fn split_table_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escape = false;

    for ch in line.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        if ch == '\\' {
            escape = true;
            continue;
        }

        if ch == '|' {
            cells.push(current.trim().to_string());
            current.clear();
            continue;
        }

        current.push(ch);
    }
    cells.push(current.trim().to_string());

    if line.trim_start().starts_with('|') && !cells.is_empty() {
        cells.remove(0);
    }
    if line.trim_end().ends_with('|') && !cells.is_empty() {
        cells.pop();
    }

    cells
}

fn build_row_node(cells: &[String], column_count: usize) -> Node {
    let mut children = Vec::new();
    for index in 0..column_count {
        let value = cells.get(index).cloned().unwrap_or_default();
        let value = value.replace("\\|", "|").replace("\\#", "#");
        let cell_children = if value.is_empty() {
            Vec::new()
        } else {
            vec![Node::Text(Text {
                position: None,
                value,
            })]
        };

        children.push(Node::TableCell(TableCell {
            position: None,
            children: cell_children,
        }));
    }

    Node::TableRow(TableRow {
        position: None,
        children,
    })
}

fn starts_blockquote(line: &str) -> bool {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return false;
    }

    line[leading_spaces..].starts_with('>')
}
