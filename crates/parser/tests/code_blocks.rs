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
fn fenced_code_omits_position_when_disabled() {
    let root = GfmParser::default().parse("```ts meta\nconsole.log(1)\n```", options());
    let Some(Node::Code(code)) = root.children.first() else {
        panic!("expected code");
    };
    assert_eq!(code.lang.as_deref(), Some("ts"));
    assert_eq!(code.meta.as_deref(), Some("meta"));
    assert_eq!(code.position, None);
}

#[test]
fn removes_fence_indentation_from_mixed_whitespace() {
    for indent_width in 1..=3 {
        let fence_indent = " ".repeat(indent_width);
        let expected_indent = if indent_width == 1 {
            "\t".to_string()
        } else {
            " ".repeat(4 - indent_width)
        };
        let root = GfmParser::default().parse(
            format!("{fence_indent}```\n \tX\n{fence_indent}```"),
            options(),
        );
        assert!(matches!(
            root.children.first(),
            Some(Node::Code(code)) if code.value == format!("{expected_indent}X\n")
        ));
    }
}

#[test]
fn recognizes_fence_after_partial_tab_in_list_item() {
    let root = GfmParser::default().parse("1234. foo\n\n\t  \t```\n\t  X\n\t  \t```", options());
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    assert!(matches!(item.children.get(1), Some(Node::Code(code)) if code.value == "X\n"));
}

#[test]
fn preserves_invalid_closing_fence_as_content() {
    for source in ["```\na\n```x", "```\na\n```x\n"] {
        let roots = [
            GfmParser::default().parse(source, options()),
            GfmExParser::default().parse(source, options()),
            YozoraParser::default().parse(source, options()),
        ];
        for root in roots {
            assert!(matches!(
                root.children.as_slice(),
                [Node::Code(code)]
                    if code.value == "a\n```x\n" && code.lang.is_none() && code.meta.is_none()
            ));
        }
    }
}

#[test]
fn indented_code_handles_space_tab_indentation_consistently() {
    for space_count in 1..=3 {
        let indent = format!("{}\t", " ".repeat(space_count));
        let root = GfmParser::default().parse(format!("{indent}foo\n{indent}bar"), options());
        assert!(matches!(
            root.children.as_slice(),
            [Node::Code(code)] if code.value == "foo\nbar\n"
        ));
    }
}
