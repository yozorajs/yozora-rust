use yozora_ast::{Node, Table, TableCell, TableColumn, TableRow, Text};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::TableToken;

pub(crate) fn parse_table_token(token: TableToken) -> BlockTokenizeResult {
    let columns = token
        .alignments
        .into_iter()
        .map(|align| TableColumn { align })
        .collect();

    let rows = token.rows.into_iter().map(build_row_node).collect();

    BlockTokenizeResult {
        node: Node::Table(Table {
            position: None,
            columns,
            children: rows,
        }),
        consumed_lines: token.consumed_lines,
    }
}

fn build_row_node(cells: Vec<String>) -> Node {
    let children = cells
        .into_iter()
        .map(|value| {
            let value = value.replace("\\|", "|").replace("\\#", "#");
            let cell_children = if value.is_empty() {
                Vec::new()
            } else {
                vec![Node::Text(Text {
                    position: None,
                    value,
                })]
            };

            Node::TableCell(TableCell {
                position: None,
                children: cell_children,
            })
        })
        .collect();

    Node::TableRow(TableRow {
        position: None,
        children,
    })
}
