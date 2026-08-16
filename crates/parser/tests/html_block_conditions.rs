use yozora_ast::Node;
use yozora_core_parser::ParseOptions;
use yozora_parser_gfm::GfmParser;

#[test]
fn condition_one_accepts_any_included_end_tag() {
    let ast = GfmParser::default().parse("<pre>\nx\n</style>\na", None);
    assert!(matches!(ast.children.first(), Some(Node::Html(_))));
    assert!(matches!(ast.children.get(1), Some(Node::Paragraph(_))));
}

#[test]
fn html_block_omits_position_when_disabled() {
    let ast = GfmParser::default().parse(
        "<div>yozora</div>",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Html(html)) = ast.children.first() else {
        panic!("expected html block");
    };
    assert_eq!(html.value, "<div>yozora</div>");
    assert_eq!(html.position, None);
}

#[test]
fn incomplete_cdata_opener_is_paragraph_text() {
    for input in ["<![CDATA\nfoo", "<![CDATAx\nfoo"] {
        let ast = GfmParser::default().parse(
            input,
            Some(ParseOptions {
                should_reserve_position: Some(false),
                ..ParseOptions::default()
            }),
        );
        assert!(matches!(
            ast.children.as_slice(),
            [Node::Paragraph(paragraph)]
                if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == input)
        ));
    }
}
