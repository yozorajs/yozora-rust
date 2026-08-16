use std::panic::{catch_unwind, AssertUnwindSafe};

use yozora_ast::{Association, Node};
use yozora_core_parser::{DefaultParser, DefaultParserProps, ParseOptions};
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer, TokenizerOptions};
use yozora_tokenizer_fenced_code::FencedCodeTokenizer;
use yozora_tokenizer_inline_code::InlineCodeTokenizer;
use yozora_tokenizer_link_reference::LinkReferenceTokenizer;
use yozora_tokenizer_paragraph::ParagraphTokenizer;
use yozora_tokenizer_text::TextTokenizer;

fn create_parser() -> DefaultParser {
    DefaultParser::new(DefaultParserProps {
        block_fallback_tokenizer: Some(Box::new(ParagraphTokenizer::default())),
        inline_fallback_tokenizer: Some(Box::new(TextTokenizer::default())),
        default_parse_options: None,
    })
}

#[test]
fn materializes_inactive_inline_delimiter_once() {
    let mut parser = create_parser();
    parser
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::default())),
            None,
        )
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(LinkReferenceTokenizer::default())),
            None,
        );

    let root = parser.parse(
        "][foo][`x`]x][bar]",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            preset_definitions: Some(vec![
                Association {
                    identifier: "foo".to_string(),
                    label: "foo".to_string(),
                },
                Association {
                    identifier: "bar".to_string(),
                    label: "bar".to_string(),
                },
            ]),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = root.children.first() else {
        panic!("expected paragraph");
    };
    assert_eq!(
        paragraph
            .children
            .iter()
            .filter(|node| matches!(node, Node::LinkReference(reference) if reference.identifier == "foo"))
            .count(),
        1
    );
}

#[test]
fn fallback_collision_does_not_mutate_registration_state() {
    let mut parser = create_parser();
    parser.use_tokenizer(
        AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::default())),
        None,
    );
    let collision = TextTokenizer::new(TokenizerOptions {
        name: Some("@yozora/tokenizer-inline-code".to_string()),
        ..TokenizerOptions::default()
    });
    assert!(catch_unwind(AssertUnwindSafe(|| {
        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(collision)));
    }))
    .is_err());
    let root = parser.parse("plain `code`", None);
    assert!(matches!(
        root.children.first(),
        Some(Node::Paragraph(paragraph))
            if paragraph.children.iter().any(|node| matches!(node, Node::InlineCode(code) if code.value == "code"))
    ));

    let mut parser = create_parser();
    parser.use_tokenizer(
        AnyTokenizer::Block(Box::new(FencedCodeTokenizer::default())),
        None,
    );
    let collision = ParagraphTokenizer::new(TokenizerOptions {
        name: Some("@yozora/tokenizer-fenced-code".to_string()),
        ..TokenizerOptions::default()
    });
    assert!(catch_unwind(AssertUnwindSafe(|| {
        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(collision)));
    }))
    .is_err());
    let root = parser.parse("plain\n\n```\ncode\n```", None);
    assert!(matches!(root.children.get(1), Some(Node::Code(_))));
}

#[test]
fn allows_replacing_fallbacks_with_same_names() {
    let mut parser = create_parser();
    parser
        .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
            ParagraphTokenizer::default(),
        )))
        .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
            TextTokenizer::default(),
        )));

    assert!(matches!(
        parser.parse("plain", None).children.first(),
        Some(Node::Paragraph(paragraph))
            if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "plain")
    ));
}

#[test]
fn unmounts_both_tokenizer_types_by_shared_name() {
    let shared_name = "shared";
    let mut parser = create_parser();
    parser
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(FencedCodeTokenizer::new(TokenizerOptions {
                name: Some(shared_name.to_string()),
                ..TokenizerOptions::default()
            }))),
            None,
        )
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::new(TokenizerOptions {
                name: Some(shared_name.to_string()),
                ..TokenizerOptions::default()
            }))),
            None,
        );

    parser.unmount_tokenizer(shared_name);
    assert!(matches!(
        parser.parse("```\ncode\n```", None).children.first(),
        Some(Node::Paragraph(_))
    ));
    assert!(matches!(
        parser.parse("`code`", None).children.first(),
        Some(Node::Paragraph(paragraph))
            if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "`code`")
    ));
}

#[test]
fn unmounts_only_the_selected_tokenizer_type() {
    let shared_name = "shared";

    let mut parser = create_parser();
    parser
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(FencedCodeTokenizer::new(TokenizerOptions {
                name: Some(shared_name.to_string()),
                ..TokenizerOptions::default()
            }))),
            None,
        )
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::new(TokenizerOptions {
                name: Some(shared_name.to_string()),
                ..TokenizerOptions::default()
            }))),
            None,
        );
    parser.unmount_block_tokenizer(shared_name);
    assert!(matches!(
        parser.parse("```\ncode\n```", None).children.first(),
        Some(Node::Paragraph(_))
    ));
    assert!(matches!(
        parser.parse("`code`", None).children.first(),
        Some(Node::Paragraph(paragraph))
            if matches!(paragraph.children.as_slice(), [Node::InlineCode(code)] if code.value == "code")
    ));

    let mut parser = create_parser();
    parser
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(FencedCodeTokenizer::new(TokenizerOptions {
                name: Some(shared_name.to_string()),
                ..TokenizerOptions::default()
            }))),
            None,
        )
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::new(TokenizerOptions {
                name: Some(shared_name.to_string()),
                ..TokenizerOptions::default()
            }))),
            None,
        );
    parser.unmount_inline_tokenizer(shared_name);
    assert!(matches!(
        parser.parse("```\ncode\n```", None).children.first(),
        Some(Node::Code(code)) if code.value == "code\n"
    ));
    assert!(matches!(
        parser.parse("`code`", None).children.first(),
        Some(Node::Paragraph(paragraph))
            if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "`code`")
    ));
}
