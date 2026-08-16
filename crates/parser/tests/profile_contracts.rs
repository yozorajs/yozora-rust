use yozora_ast::Node;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;

#[test]
fn chunked_input_is_independent_of_boundaries() {
    let content = "a\r\nb😀c";
    let chunks = content
        .chars()
        .map(|character| character.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        GfmParser::default().parse(chunks, None),
        GfmParser::default().parse(content, None)
    );
}

#[test]
fn recognizes_block_tokenizers_after_partial_tab_indentation() {
    for (source, expected) in [
        ("# bar", "heading"),
        ("***", "thematicBreak"),
        ("> bar", "blockquote"),
        ("- bar", "list"),
    ] {
        let root = GfmParser::default().parse(format!("1234. foo\n\t  \t{source}"), None);
        let Some(Node::List(list)) = root.children.first() else {
            panic!("expected list");
        };
        let Some(Node::ListItem(item)) = list.children.first() else {
            panic!("expected list item");
        };
        assert_eq!(
            item.children.get(1).map(Node::node_type),
            Some(expected),
            "source={source}, item={item:#?}"
        );
    }

    let root = GfmParser::default().parse("1234. foo\n\n\t  \t<div>bar</div>", None);
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    assert!(matches!(item.children.get(1), Some(Node::Html(_))));

    let root = GfmParser::default().parse("1234. foo\n\n\t  \t[a]: /url\n\n[a]", None);
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    assert!(matches!(item.children.get(1), Some(Node::Definition(_))));
    assert!(matches!(
        root.children.last(),
        Some(Node::Paragraph(paragraph))
            if matches!(paragraph.children.first(), Some(Node::LinkReference(_)))
    ));

    let root = GfmParser::default().parse("1234. foo\n\n\t  bar\n\t  \t---", None);
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    assert!(matches!(item.children.get(1), Some(Node::Heading(heading)) if heading.depth == 2));
}

#[test]
fn recognizes_yozora_extensions_after_partial_tab_indentation() {
    let root = YozoraParser::default().parse(
        "1234. foo\n\n\t  header | value\n\t  \t--- | ---\n\t  cell | data",
        None,
    );
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    assert!(
        matches!(item.children.get(1), Some(Node::Table(_))),
        "item={item:#?}"
    );

    let root = YozoraParser::default().parse(
        "1234. foo\n\n\t  \timport Parser from '@yozora/parser'",
        None,
    );
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    assert!(matches!(item.children.get(1), Some(Node::EcmaImport(_))));
}

#[test]
fn positions_use_utf16_units_and_terminal_line_endings() {
    let reserve_position = || {
        Some(ParseOptions {
            should_reserve_position: Some(true),
            ..ParseOptions::default()
        })
    };
    let root = GfmParser::default().parse("😀", reserve_position());
    let position = root.position.as_ref().expect("expected root position");
    assert_eq!((position.start.column, position.start.offset), (1, Some(0)));
    assert_eq!((position.end.column, position.end.offset), (3, Some(2)));
    let Some(Node::Paragraph(paragraph)) = root.children.first() else {
        panic!("expected paragraph");
    };
    let Some(Node::Text(text)) = paragraph.children.first() else {
        panic!("expected text");
    };
    let position = text.position.as_ref().expect("expected text position");
    assert_eq!((position.end.column, position.end.offset), (3, Some(2)));

    for (input, expected) in [
        ("a\n", (2, 1, Some(2))),
        ("a\r", (2, 1, Some(2))),
        ("a\r\n", (2, 1, Some(3))),
    ] {
        let root = GfmParser::default().parse(input, reserve_position());
        let end = &root.position.as_ref().expect("expected root position").end;
        assert_eq!(
            (end.line, end.column, end.offset),
            expected,
            "input={input:?}"
        );
    }
}

#[test]
fn failed_multiline_definition_is_reprocessed_before_following_blocks() {
    for (source, expected_types) in [
        ("[\n#\n\n", vec!["paragraph", "heading"]),
        ("[\r\n## x\r\n\r\n", vec!["paragraph", "heading"]),
        ("[\n***\n\n", vec!["paragraph", "thematicBreak"]),
    ] {
        let root = GfmParser::default().parse(source, None);
        assert_eq!(
            root.children
                .iter()
                .map(Node::node_type)
                .collect::<Vec<_>>(),
            expected_types,
            "source={source:?}"
        );
    }

    let root = GfmParser::default().parse("> [\n> #\n>\n", None);
    let Some(Node::Blockquote(blockquote)) = root.children.first() else {
        panic!("expected blockquote");
    };
    assert!(matches!(
        blockquote.children.first(),
        Some(Node::Paragraph(_))
    ));
    assert!(
        matches!(blockquote.children.get(1), Some(Node::Heading(heading)) if heading.depth == 1)
    );
}
