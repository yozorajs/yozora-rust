use yozora_ast::{AlignType, Node, Table};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

fn options() -> Option<ParseOptions> {
    Some(ParseOptions {
        should_reserve_position: Some(false),
        ..ParseOptions::default()
    })
}

fn first_table(root: &yozora_ast::Root) -> &Table {
    let Some(Node::Table(table)) = root.children.first() else {
        panic!("expected table");
    };
    table
}

#[test]
fn preserves_trailing_backslash_in_final_cell() {
    let without_line_ending = YozoraParser::default().parse("a|\n-|\n\\", options());
    let with_line_ending = YozoraParser::default().parse("a|\n-|\n\\\n", options());
    assert_eq!(without_line_ending, with_line_ending);

    let table = first_table(&without_line_ending);
    let Some(Node::TableRow(row)) = table.children.get(1) else {
        panic!("expected body row");
    };
    let Some(Node::TableCell(cell)) = row.children.first() else {
        panic!("expected cell");
    };
    assert!(matches!(cell.children.as_slice(), [Node::Text(text)] if text.value == "\\"));
}

#[test]
fn recognizes_pipe_less_single_column_alignment() {
    for (delimiter, align) in [
        (":---", AlignType::Left),
        ("---:", AlignType::Right),
        (":---:", AlignType::Center),
    ] {
        let root = YozoraParser::default().parse(format!("foo\n{delimiter}"), options());
        let table = first_table(&root);
        assert_eq!(table.columns.len(), 1);
        assert_eq!(table.columns[0].align, Some(align));
        let Some(Node::TableRow(row)) = table.children.first() else {
            panic!("expected header row");
        };
        let Some(Node::TableCell(cell)) = row.children.first() else {
            panic!("expected header cell");
        };
        assert!(matches!(cell.children.as_slice(), [Node::Text(text)] if text.value == "foo"));
    }
}

#[test]
fn preserves_setext_heading_precedence() {
    let root = YozoraParser::default().parse("foo\n---", options());
    assert!(matches!(
        root.children.first(),
        Some(Node::Heading(heading))
            if heading.depth == 2
                && matches!(heading.children.as_slice(), [Node::Text(text)] if text.value == "foo")
    ));
}

#[test]
fn recognizes_empty_header_cells() {
    for (source, values) in [
        ("| |\n| --- |", vec![""]),
        ("||\n| --- |", vec![""]),
        ("| | Name |\n| --- | --- |", vec!["", "Name"]),
    ] {
        let root = YozoraParser::default().parse(source, options());
        let table = first_table(&root);
        assert_eq!(table.columns.len(), values.len());
        let Some(Node::TableRow(row)) = table.children.first() else {
            panic!("expected header row");
        };
        assert_eq!(row.children.len(), values.len());

        for (cell, expected) in row.children.iter().zip(values) {
            let Node::TableCell(cell) = cell else {
                panic!("expected table cell");
            };
            if expected.is_empty() {
                assert!(cell.children.is_empty());
            } else {
                assert!(
                    matches!(cell.children.as_slice(), [Node::Text(text)] if text.value == expected)
                );
            }
        }
    }
}

#[test]
fn lone_pipe_is_not_an_empty_header_cell() {
    let root = YozoraParser::default().parse("|\n| --- |", options());
    assert!(matches!(root.children.first(), Some(Node::Paragraph(_))));
}
