use yozora_ast::{AlignType, Node, TABLE_TYPE};

use crate::util::create_character_escaper;
use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct TableWeaver;

fn separator(align: Option<AlignType>, width: usize) -> String {
    match align {
        Some(AlignType::Center) => format!(":{}:", "-".repeat(width.saturating_sub(2))),
        Some(AlignType::Left) => format!(":{}", "-".repeat(width.saturating_sub(1))),
        Some(AlignType::Right) => format!("{}:", "-".repeat(width.saturating_sub(1))),
        None => "-".repeat(width),
    }
}

impl NodeWeaver for TableWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![TABLE_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave<'a>(
        &self,
        node: &'a Node,
        context: &NodeMarkupWeaveContext<'a>,
        _: usize,
    ) -> NodeMarkup {
        let Node::Table(node) = node else {
            unreachable!()
        };
        let Some(Node::TableRow(header)) = node.children.first() else {
            return NodeMarkup::default();
        };
        let escape = create_character_escaper(&['|']);
        let head = header
            .children
            .iter()
            .map(|cell| match cell {
                Node::TableCell(cell) => escape(&context.weave_inline_nodes(&cell.children)),
                _ => String::new(),
            })
            .collect::<Vec<_>>();
        let column_count = head.len();
        let mut widths = head.iter().map(|cell| cell.len()).collect::<Vec<_>>();
        let mut body = Vec::new();
        for row in node.children.iter().skip(1) {
            let Node::TableRow(row) = row else {
                continue;
            };
            let mut columns = Vec::with_capacity(column_count);
            for (index, width) in widths.iter_mut().enumerate().take(column_count) {
                let value = match &row.children[index] {
                    Node::TableCell(cell) => escape(&context.weave_inline_nodes(&cell.children)),
                    _ => String::new(),
                };
                *width = (*width).max(value.len());
                columns.push(value);
            }
            body.push(columns);
        }
        for width in &mut widths {
            let odd_width = *width | 1;
            *width |= if odd_width < 3 { 3 } else { odd_width };
        }
        let content = if column_count == 1 {
            let width = widths[0];
            let separator = format!("|{}|", separator(node.columns[0].align, width + 2));
            format!(
                "| {:width$} |\n{separator}\n{}",
                head[0],
                body.iter()
                    .map(|row| format!("| {:width$} |", row[0]))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            let separator_line = node
                .columns
                .iter()
                .enumerate()
                .map(|(index, column)| {
                    separator(column.align, widths[index] + usize::from(index > 0) + 1)
                })
                .collect::<Vec<_>>()
                .join("|");
            let format_row = |row: &[String]| {
                row.iter()
                    .enumerate()
                    .map(|(index, cell)| {
                        if index + 1 == column_count {
                            cell.clone()
                        } else {
                            format!("{cell:width$}", width = widths[index])
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            format!(
                "{}\n{separator_line}\n{}",
                format_row(&head),
                body.iter()
                    .map(|row| format_row(row))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        NodeMarkup {
            opener: Some(content),
            content: Some(String::new()),
            ..NodeMarkup::default()
        }
    }
}
