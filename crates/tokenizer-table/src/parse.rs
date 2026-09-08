use yozora_ast::{Node, NodeBuffer, Table, TableCell, TableRow};
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::{merge_and_strip_content_lines, BlockToken, ParseBlockPhaseApi};

use crate::types::TableTokenData;

pub(crate) fn parse_table_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = NodeBuffer::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<TableTokenData>() else {
            continue;
        };

        let rows = data
            .rows
            .iter()
            .map(|row| {
                let cells = row
                    .cells
                    .iter()
                    .map(|cell| {
                        let merged =
                            merge_and_strip_content_lines(&cell.lines, 0, cell.lines.len());
                        let contents = unescape_table_cell_contents(&merged);
                        let children = parse_api.process_inlines(&contents);

                        Node::TableCell(TableCell {
                            position: if parse_api.should_reserve_position() {
                                Some(cell.position.clone())
                            } else {
                                None
                            },
                            children,
                        })
                    })
                    .collect::<NodeBuffer>();

                Node::TableRow(TableRow {
                    position: if parse_api.should_reserve_position() {
                        Some(row.position.clone())
                    } else {
                        None
                    },
                    children: cells.into_vec(),
                })
            })
            .collect::<NodeBuffer>();

        nodes.push(Node::Table(Table {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            columns: data.columns.clone(),
            children: rows.into_vec(),
        }));
    }

    nodes.into_vec()
}

fn unescape_table_cell_contents(node_points: &[NodePoint]) -> Vec<NodePoint> {
    let mut contents = Vec::new();
    let mut i = 0usize;
    while i < node_points.len() {
        let point = node_points[i];
        if point.code_point == AsciiCodePoint::BACKSLASH as i32 && i + 1 < node_points.len() {
            let next = node_points[i + 1];
            if next.code_point != AsciiCodePoint::VERTICAL_SLASH as i32 {
                contents.push(point);
            }
            contents.push(next);
            i += 2;
            continue;
        }

        contents.push(point);
        i += 1;
    }

    contents
}
