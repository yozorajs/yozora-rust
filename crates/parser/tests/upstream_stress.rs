use yozora_ast::Node;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

#[test]
fn parses_deeply_nested_images_without_stack_overflow() {
    let depth = std::env::var("YOZORA_STRESS_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let source = format!("{}x{}", "![".repeat(depth), "](/url)".repeat(depth));
    let ast = YozoraParser::default().parse(
        source,
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph")
    };
    assert!(matches!(paragraph.children.as_slice(), [Node::Image(image)]
        if image.alt == "x" && image.url == "/url"));
}

#[test]
fn parses_nested_images_and_references_with_real_alt_text() {
    let depth = 4_000;
    let suffix: String = (0..depth)
        .map(|level| if level % 2 == 0 { "](/url)" } else { "][ref]" })
        .collect();
    let source = format!("{}**x**{}\n\n[ref]: /ref", "![".repeat(depth), suffix);
    let ast = YozoraParser::default().parse(source, None);
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph")
    };
    assert!(
        matches!(paragraph.children.as_slice(), [Node::ImageReference(image)]
        if image.alt == "x" && image.identifier == "ref")
    );
}

#[test]
fn materializes_and_drops_deep_inline_children_in_image_alt_text() {
    let depth = 4_000;
    let content = format!("{}x{}", "**".repeat(depth), "**".repeat(depth));
    let parser = YozoraParser::default();
    for reference in [false, true] {
        let source = if reference {
            format!("![{content}][ref]\n\n[ref]: /ref")
        } else {
            format!("![{content}](/url)")
        };
        let ast = parser.parse(source, None);
        let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
            panic!("expected paragraph")
        };
        match paragraph.children.as_slice() {
            [Node::Image(image)] => assert_eq!(image.alt, "x"),
            [Node::ImageReference(image)] => assert_eq!(image.alt, "x"),
            _ => panic!("expected image"),
        }
    }
}

#[test]
fn parses_deeply_nested_blocks_without_stack_overflow() {
    let depth = std::env::var("YOZORA_STRESS_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let source = format!("{}x", "> ".repeat(depth));
    let ast = YozoraParser::default().parse(
        source,
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );

    let mut node = ast.children.first().expect("expected nested blockquote");
    for _ in 0..depth {
        let Node::Blockquote(blockquote) = node else {
            panic!("expected blockquote")
        };
        node = blockquote
            .children
            .first()
            .expect("expected nested block child");
    }
    assert!(matches!(node, Node::Paragraph(_)));
}

#[test]
fn handles_unmatched_cross_tokenizer_delimiters_linearly() {
    let count = std::env::var("YOZORA_CROSS_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000);
    let source = "[x]: <".repeat(count);
    let mut parser = YozoraParser::default();
    if let Ok(names) = std::env::var("YOZORA_UNMOUNT") {
        for name in names.split(',').filter(|name| !name.is_empty()) {
            parser.unmount_tokenizer(name);
        }
    }
    let ast = parser.parse(
        source.clone(),
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph")
    };
    assert!(matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == source));
}

#[test]
fn materializes_wide_sibling_lists() {
    let count = std::env::var("YOZORA_WIDE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(150_000);
    let ast = YozoraParser::default().parse(
        "x\n\n".repeat(count),
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    assert_eq!(ast.children.len(), count);
    assert!(matches!(ast.children.first(), Some(Node::Paragraph(_))));
    assert!(matches!(ast.children.last(), Some(Node::Paragraph(_))));
}

#[test]
fn materializes_wide_sibling_lists_inside_container() {
    let count = std::env::var("YOZORA_WIDE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(150_000);
    let ast = YozoraParser::default().parse(
        "> x\n>\n".repeat(count),
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Blockquote(blockquote)) = ast.children.first() else {
        panic!("expected blockquote")
    };
    assert_eq!(ast.children.len(), 1);
    assert_eq!(blockquote.children.len(), count);
    assert!(matches!(
        blockquote.children.first(),
        Some(Node::Paragraph(_))
    ));
    assert!(matches!(
        blockquote.children.last(),
        Some(Node::Paragraph(_))
    ));
}
