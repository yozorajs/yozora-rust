use yozora_ast::{Node, Root};
use yozora_core_parser::ParseOptions;
use yozora_core_tokenizer::AnyTokenizer;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;
use yozora_tokenizer_delete::DeleteTokenizer;
use yozora_tokenizer_inline_code::INLINE_CODE_TOKENIZER_NAME;
use yozora_tokenizer_inline_math::{InlineMathTokenizer, InlineMathTokenizerOptions};

fn without_positions() -> Option<ParseOptions> {
    Some(ParseOptions {
        should_reserve_position: Some(false),
        ..ParseOptions::default()
    })
}

fn paragraph_children(root: &Root) -> &[Node] {
    let Some(Node::Paragraph(paragraph)) = root.children.first() else {
        panic!("expected paragraph, root={root:#?}");
    };
    &paragraph.children
}

#[test]
fn standard_autolinks_preserve_unicode_labels_and_backslashes() {
    for (source, expected_url, expected_label) in [
        (
            "<https://example.com/路径>",
            "https://example.com/%E8%B7%AF%E5%BE%84",
            "https://example.com/路径",
        ),
        ("<foo:😀>", "foo:%F0%9F%98%80", "foo:😀"),
        ("<foo:a\\*b>", "foo:a%5C*b", "foo:a\\*b"),
    ] {
        let root = GfmParser::default().parse(source, without_positions());
        assert!(matches!(
            paragraph_children(&root),
            [Node::Link(link)]
                if link.url == expected_url
                    && matches!(link.children.as_slice(), [Node::Text(text)] if text.value == expected_label)
        ));
    }
}

#[test]
fn unicode_punctuation_does_not_open_emphasis() {
    for punctuation in ['\u{061d}', '\u{11f43}'] {
        let source = format!("a*{punctuation}foo*");
        let root = GfmParser::default().parse(source.as_str(), without_positions());
        assert!(matches!(
            paragraph_children(&root),
            [Node::Text(text)] if text.value == source
        ));
    }
}

#[test]
fn delete_matches_only_one_or_two_balanced_tildes_and_omits_position() {
    let mut gfm = GfmParser::default();
    gfm.use_tokenizer(
        AnyTokenizer::Inline(Box::new(DeleteTokenizer::default())),
        None,
    );

    for source in ["~deleted~", "~~deleted~~"] {
        let root = gfm.parse(source, without_positions());
        assert!(matches!(
            paragraph_children(&root),
            [Node::Delete(delete)]
                if delete.position.is_none()
                    && matches!(delete.children.as_slice(), [Node::Text(text)] if text.value == "deleted")
        ));
    }
    for source in [
        "This will ~~~not~~~ strike.",
        "~not deleted~~",
        "~~not deleted~",
    ] {
        let root = gfm.parse(source, without_positions());
        assert!(matches!(
            paragraph_children(&root),
            [Node::Text(text)] if text.value == source
        ));
    }

    let root = GfmExParser::default().parse("~~Hi~~ Hello, ~there~ world!", without_positions());
    assert!(matches!(
        paragraph_children(&root),
        [Node::Delete(first), Node::Text(space), Node::Delete(second), Node::Text(tail)]
            if matches!(first.children.as_slice(), [Node::Text(text)] if text.value == "Hi")
                && space.value == " Hello, "
                && matches!(second.children.as_slice(), [Node::Text(text)] if text.value == "there")
                && tail.value == " world!"
    ));
}

#[test]
fn hard_breaks_own_line_endings_and_soft_breaks_stay_in_text() {
    for source in ["foo  \nbar", "foo\\\nbar", "foo  \r\nbar"] {
        let root = GfmParser::default().parse(source, without_positions());
        assert!(matches!(
            paragraph_children(&root),
            [Node::Text(before), Node::Break(_), Node::Text(after)]
                if before.value == "foo" && after.value == "bar"
        ));
    }

    let root = GfmParser::default().parse("foo\nbar", without_positions());
    assert!(matches!(
        paragraph_children(&root),
        [Node::Text(text)] if text.value == "foo\nbar"
    ));
}

#[test]
fn hard_break_positions_include_the_line_ending() {
    for (source, line_start_offset, text_column, text_offset) in [
        ("foo  \nbar", 6, 1, 6),
        ("foo  \r\nbar", 7, 1, 7),
        ("foo  \n     bar", 6, 6, 11),
    ] {
        let root = GfmParser::default().parse(
            source,
            Some(ParseOptions {
                should_reserve_position: Some(true),
                ..ParseOptions::default()
            }),
        );
        let [Node::Text(_), Node::Break(line_break), Node::Text(text)] = paragraph_children(&root)
        else {
            panic!("expected text/break/text, root={root:#?}");
        };
        let break_position = line_break.position.as_ref().expect("break position");
        assert_eq!(
            (
                break_position.start.line,
                break_position.start.column,
                break_position.start.offset,
            ),
            (1, 4, Some(3))
        );
        assert_eq!(
            (
                break_position.end.line,
                break_position.end.column,
                break_position.end.offset,
            ),
            (2, 1, Some(line_start_offset))
        );
        let text_position = text.position.as_ref().expect("text position");
        assert_eq!(
            (
                text_position.start.line,
                text_position.start.column,
                text_position.start.offset
            ),
            (2, text_column, Some(text_offset))
        );
        assert_eq!(text.value, "bar");
    }
}

#[test]
fn inline_code_handles_unmatched_candidates_and_preserves_tabs() {
    let source = "\\``x".repeat(10_000);
    let root = GfmParser::default().parse(source.as_str(), without_positions());
    assert!(matches!(
        paragraph_children(&root),
        [Node::Text(text)] if text.value == "``x".repeat(10_000)
    ));

    for (source, expected) in [("`\tfoo\t`", "\tfoo\t"), ("` \t `", "\t")] {
        let root = GfmParser::default().parse(source, without_positions());
        assert!(
            matches!(
                paragraph_children(&root),
                [Node::InlineCode(code)] if code.value == expected
            ),
            "source={source:?}, root={root:#?}"
        );
    }
}

#[test]
fn inline_math_handles_unmatched_candidates_and_preserves_tabs() {
    let mut parser = GfmParser::default();
    parser.unmount_inline_tokenizer(INLINE_CODE_TOKENIZER_NAME);
    parser.use_tokenizer(
        AnyTokenizer::Inline(Box::new(InlineMathTokenizer::new(
            InlineMathTokenizerOptions {
                backtick_required: true,
                ..InlineMathTokenizerOptions::default()
            },
        ))),
        Some(INLINE_CODE_TOKENIZER_NAME),
    );
    let value = "`$x".repeat(10_000);
    let root = parser.parse(value.as_str(), without_positions());
    assert!(
        matches!(
            paragraph_children(&root),
            [Node::Text(text)] if text.value == value
        ),
        "root={root:#?}"
    );

    for source in ["$\tfoo\t$", "`$\tfoo\t$`"] {
        let root = YozoraParser::default().parse(source, without_positions());
        assert!(matches!(
            paragraph_children(&root),
            [Node::InlineMath(math)] if math.value == "\tfoo\t"
        ));
    }
}
