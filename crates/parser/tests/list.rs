use yozora_ast::{List, ListItem, Node, Root, TaskStatus};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;

fn options() -> Option<ParseOptions> {
    Some(ParseOptions {
        should_reserve_position: Some(false),
        ..ParseOptions::default()
    })
}

fn first_list(root: &Root) -> &List {
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    list
}

fn first_item(list: &List) -> &ListItem {
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    item
}

#[test]
fn handles_space_tab_indentation_in_list_items() {
    for space_count in 1..=3 {
        let indent = format!("{}\t\t", " ".repeat(space_count));
        let root = GfmParser::default().parse(format!("- foo\n\n{indent}bar"), options());
        let item = first_item(first_list(&root));

        assert!(matches!(
            item.children.first(),
            Some(Node::Paragraph(paragraph))
                if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "foo")
        ));
        assert!(
            matches!(
                item.children.get(1),
                Some(Node::Code(code)) if code.value == "  bar\n"
            ),
            "space_count={space_count}, item={item:#?}"
        );
    }
}

#[test]
fn matches_continuation_indentation_by_columns() {
    let root = GfmParser::default().parse("123. foo\n\n  \tbar", options());
    assert!(matches!(root.children.first(), Some(Node::List(_))));
    assert!(matches!(root.children.get(1), Some(Node::Code(code)) if code.value == "bar\n"));
}

#[test]
fn tracks_tab_columns_across_nested_containers() {
    let root = GfmParser::default().parse("> - foo\n>\n>  \t\tbar", options());
    let Some(Node::Blockquote(blockquote)) = root.children.first() else {
        panic!("expected blockquote");
    };
    let Some(Node::List(list)) = blockquote.children.first() else {
        panic!("expected nested list");
    };
    let item = first_item(list);

    assert!(matches!(item.children.get(1), Some(Node::Code(code)) if code.value == "bar\n"));
}

#[test]
fn preserves_tabs_not_consumed_by_list_indentation() {
    let root = GfmParser::default().parse("1234. ```\n\t  \tX\n      ```", options());
    let item = first_item(first_list(&root));
    assert!(matches!(item.children.first(), Some(Node::Code(code)) if code.value == "\tX\n"));
}

#[test]
fn handles_tabs_after_list_markers_by_column_width() {
    for (indent_width, initial_indent, expected_indent) in [
        (0, "", "  "),
        (1, " ", " "),
        (2, "  ", ""),
        (3, "   ", "   "),
    ] {
        let root = GfmParser::default().parse(format!("{initial_indent}-\t\tfoo"), options());
        let item = first_item(first_list(&root));
        assert!(
            matches!(item.children.first(), Some(Node::Code(code)) if code.value == format!("{expected_indent}foo\n")),
            "indent_width={indent_width}"
        );
    }
}

#[test]
fn indented_empty_items_do_not_interrupt_paragraphs() {
    for (indent_width, initial_indent) in [(0, ""), (1, " "), (2, "  "), (3, "   ")] {
        let root = GfmParser::default().parse(format!("foo\n{initial_indent}+\t"), options());
        assert_eq!(root.children.len(), 1, "indent_width={indent_width}");
        assert!(matches!(
            root.children.first(),
            Some(Node::Paragraph(paragraph))
                if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "foo\n+")
        ));
    }
}

#[test]
fn task_like_non_empty_item_interrupts_paragraph() {
    let root = GfmParser::default().parse("foo\n- [ ]", options());
    assert!(matches!(root.children.first(), Some(Node::Paragraph(_))));
    assert!(matches!(root.children.get(1), Some(Node::List(_))));
}

#[test]
fn consumes_whitespace_after_task_markers() {
    for whitespace in ["     ", "\t", "\t\t"] {
        let root = YozoraParser::default().parse(format!("- [ ]{whitespace}foo"), options());
        let item = first_item(first_list(&root));
        assert_eq!(item.status, Some(TaskStatus::Todo));
        assert!(matches!(item.children.as_slice(), [Node::Text(text)] if text.value == "foo"));
    }
}

#[test]
fn keeps_task_content_aligned_to_list_indentation() {
    let continuation = YozoraParser::default().parse("- [ ]     foo\n  bar", options());
    let item = first_item(first_list(&continuation));
    assert!(matches!(item.children.as_slice(), [Node::Text(text)] if text.value == "foo\nbar"));

    let nested = YozoraParser::default().parse("- [ ]     parent\n  - child", options());
    let item = first_item(first_list(&nested));
    assert!(matches!(item.children.first(), Some(Node::Text(_))));
    assert!(matches!(item.children.get(1), Some(Node::List(_))));

    let code = YozoraParser::default().parse("- [ ]     foo\n\n      code", options());
    let item = first_item(first_list(&code));
    assert!(matches!(item.children.first(), Some(Node::Paragraph(_))));
    assert!(matches!(item.children.get(1), Some(Node::Code(code)) if code.value == "code\n"));
}
