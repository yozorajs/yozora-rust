use yozora_ast::Node;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;

fn options() -> Option<ParseOptions> {
    Some(ParseOptions {
        should_reserve_position: Some(false),
        ..ParseOptions::default()
    })
}

#[test]
fn blockquote_handles_tabs_after_markers() {
    let root = GfmParser::default().parse(">\t\tfoo\n>\t\tbar", options());
    let Some(Node::Blockquote(blockquote)) = root.children.first() else {
        panic!("expected blockquote");
    };
    assert!(matches!(
        blockquote.children.as_slice(),
        [Node::Code(code)] if code.value == "  foo\n  bar\n"
    ));

    for (indent_width, initial_indent, expected_indent) in [
        (0, "", "  "),
        (1, " ", " "),
        (2, "  ", ""),
        (3, "   ", "   "),
    ] {
        let root = GfmParser::default().parse(format!("{initial_indent}>\t\tfoo"), options());
        let Some(Node::Blockquote(blockquote)) = root.children.first() else {
            panic!("expected blockquote");
        };
        assert!(
            matches!(blockquote.children.as_slice(), [Node::Code(code)] if code.value == format!("{expected_indent}foo\n")),
            "indent_width={indent_width}"
        );
    }
}

#[test]
fn rejects_unspaced_atx_heading_content() {
    for source in ["#x", "#x\n"] {
        for root in [
            GfmParser::default().parse(source, options()),
            GfmExParser::default().parse(source, options()),
            YozoraParser::default().parse(source, options()),
        ] {
            assert!(matches!(
                root.children.as_slice(),
                [Node::Paragraph(paragraph)]
                    if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "#x")
            ));
        }
    }
}

#[test]
fn setext_underlines_allow_ascii_tabs_but_not_nbsp() {
    for source in ["foo\n---\u{00A0}", "foo\n---\u{00A0}\n"] {
        for root in [
            GfmParser::default().parse(source, options()),
            GfmExParser::default().parse(source, options()),
            YozoraParser::default().parse(source, options()),
        ] {
            assert!(matches!(root.children.first(), Some(Node::Paragraph(_))));
        }
    }

    for root in [
        GfmParser::default().parse("foo\n---\t", options()),
        GfmExParser::default().parse("foo\n---\t", options()),
        YozoraParser::default().parse("foo\n---\t", options()),
    ] {
        assert!(matches!(
            root.children.as_slice(),
            [Node::Heading(heading)]
                if heading.depth == 2
                    && matches!(heading.children.as_slice(), [Node::Text(text)] if text.value == "foo")
        ));
    }
}

#[test]
fn thematic_breaks_allow_ascii_tabs_but_not_nbsp() {
    for source in ["*\u{00A0}*\u{00A0}*", "*\u{00A0}*\u{00A0}*\n"] {
        for root in [
            GfmParser::default().parse(source, options()),
            GfmExParser::default().parse(source, options()),
            YozoraParser::default().parse(source, options()),
        ] {
            assert!(matches!(root.children.first(), Some(Node::Paragraph(_))));
        }
    }

    for root in [
        GfmParser::default().parse("*\t*\t*", options()),
        GfmExParser::default().parse("*\t*\t*", options()),
        YozoraParser::default().parse("*\t*\t*", options()),
    ] {
        assert!(matches!(root.children.as_slice(), [Node::ThematicBreak(_)]));
    }
}

#[test]
fn text_and_paragraph_omit_positions_when_disabled() {
    let root = GfmParser::default().parse("foo", options());
    let Some(Node::Paragraph(paragraph)) = root.children.first() else {
        panic!("expected paragraph");
    };
    assert_eq!(paragraph.position, None);
    assert!(matches!(
        paragraph.children.as_slice(),
        [Node::Text(text)] if text.value == "foo" && text.position.is_none()
    ));
}

#[test]
fn soft_breaks_preserve_unicode_whitespace() {
    for whitespace in ["\u{00A0}", "\u{2003}"] {
        let root =
            GfmParser::default().parse(format!("foo{whitespace}\n{whitespace}bar"), options());
        let Some(Node::Paragraph(paragraph)) = root.children.first() else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            paragraph.children.as_slice(),
            [Node::Text(text)] if text.value == format!("foo{whitespace}\n{whitespace}bar")
        ));
    }
}

#[test]
fn soft_breaks_remove_ascii_space_and_tab() {
    for whitespace in [" ", "\t"] {
        let root =
            GfmParser::default().parse(format!("foo{whitespace}\n{whitespace}bar"), options());
        let Some(Node::Paragraph(paragraph)) = root.children.first() else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            paragraph.children.as_slice(),
            [Node::Text(text)] if text.value == "foo\nbar"
        ));
    }
}
