use std::sync::{Arc, Mutex};

use yozora_ast::{Node, Root};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;

fn paragraph_children(root: &Root) -> &[Node] {
    let Some(Node::Paragraph(paragraph)) = root.children.first() else {
        panic!("expected paragraph");
    };
    &paragraph.children
}

#[test]
fn link_omits_position_when_disabled() {
    let root = GfmParser::default().parse(
        "[home](https://example.com \"site\")",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Link(link)) = paragraph_children(&root).first() else {
        panic!("expected link");
    };

    assert_eq!(link.url, "https://example.com");
    assert_eq!(link.title.as_deref(), Some("site"));
    assert_eq!(link.position, None);
}

#[test]
fn empty_link_destinations_are_formatted() {
    for source in ["[x]()", "[x](<>)"] {
        let formatted_urls = Arc::new(Mutex::new(Vec::new()));
        let recorded_urls = Arc::clone(&formatted_urls);
        let root = GfmParser::default().parse(
            source,
            Some(ParseOptions {
                should_reserve_position: Some(false),
                format_url: Some(Arc::new(move |url| {
                    recorded_urls
                        .lock()
                        .expect("url recorder lock")
                        .push(url.to_string());
                    format!("formatted:{url}")
                })),
                ..ParseOptions::default()
            }),
        );
        let Some(Node::Link(link)) = paragraph_children(&root).first() else {
            panic!("expected link for {source}");
        };

        assert_eq!(
            *formatted_urls.lock().expect("url recorder lock"),
            vec![String::new()],
            "source={source}"
        );
        assert_eq!(link.url, "formatted:");
    }
}

#[test]
fn rejects_destination_whitespace_after_backslash() {
    for source in [
        "[x](a\\ b)",
        "[x](<a\\\nb>)",
        "![x](a\\ b)",
        "![x](<a\\\nb>)",
    ] {
        let formatted_urls = Arc::new(Mutex::new(Vec::new()));
        let recorded_urls = Arc::clone(&formatted_urls);
        GfmParser::default().parse(
            source,
            Some(ParseOptions {
                should_reserve_position: Some(false),
                format_url: Some(Arc::new(move |url| {
                    recorded_urls
                        .lock()
                        .expect("url recorder lock")
                        .push(url.to_string());
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
        );

        assert!(
            formatted_urls.lock().expect("url recorder lock").is_empty(),
            "source={source:?}"
        );
    }
}

#[test]
fn resolves_inline_links_and_autolinks_in_source_order() {
    for (source, expected) in [
        (
            "[x](/u) <https://example.com/a>",
            vec![
                ("link", "/u"),
                ("text", " "),
                ("link", "https://example.com/a"),
            ],
        ),
        (
            "<https://example.com/a> [x](/u)",
            vec![
                ("link", "https://example.com/a"),
                ("text", " "),
                ("link", "/u"),
            ],
        ),
        (
            "[x](<https://example.com/a>)",
            vec![("link", "https://example.com/a")],
        ),
    ] {
        let root = GfmParser::default().parse(
            source,
            Some(ParseOptions {
                should_reserve_position: Some(false),
                ..ParseOptions::default()
            }),
        );
        let actual: Vec<(&str, &str)> = paragraph_children(&root)
            .iter()
            .map(|node| match node {
                Node::Link(link) => ("link", link.url.as_str()),
                Node::Text(text) => ("text", text.value.as_str()),
                _ => panic!("unexpected node for {source}: {node:?}"),
            })
            .collect();

        assert_eq!(actual, expected, "source={source}");
    }
}

#[test]
fn limits_nested_parentheses_in_link_and_image_destinations() {
    for (label, expected_node_type) in [("[x]", "link"), ("![x]", "image")] {
        let accepted_source = format!("{label}({}url{})", "(".repeat(32), ")".repeat(32));
        let rejected_source = format!("{label}({}url{})", "(".repeat(33), ")".repeat(33));

        let accepted = GfmParser::default().parse(
            accepted_source,
            Some(ParseOptions {
                should_reserve_position: Some(false),
                ..ParseOptions::default()
            }),
        );
        let rejected = GfmParser::default().parse(
            rejected_source,
            Some(ParseOptions {
                should_reserve_position: Some(false),
                ..ParseOptions::default()
            }),
        );

        assert!(paragraph_children(&accepted).iter().any(|node| matches!(
            (expected_node_type, node),
            ("link", Node::Link(_)) | ("image", Node::Image(_))
        )));
        assert!(!paragraph_children(&rejected).iter().any(|node| matches!(
            (expected_node_type, node),
            ("link", Node::Link(_)) | ("image", Node::Image(_))
        )));
    }
}

#[test]
fn preserves_reference_url_escaping() {
    for (source, expected) in [
        (
            "https://example.com/%2Fadmin",
            "https://example.com/%2Fadmin",
        ),
        (
            "https://example.com/%252Fadmin",
            "https://example.com/%252Fadmin",
        ),
        ("https://example.com/%23frag", "https://example.com/%23frag"),
        (
            "https://example.com/%3Fq%3D1",
            "https://example.com/%3Fq%3D1",
        ),
        ("foo%20b&auml;", "foo%20b%C3%A4"),
        ("100%", "100%"),
        ("%ZZ", "%ZZ"),
        ("foo%2", "foo%2"),
    ] {
        let root = GfmParser::default().parse(
            format!("[x]({source})"),
            Some(ParseOptions {
                should_reserve_position: Some(false),
                ..ParseOptions::default()
            }),
        );
        let Some(Node::Link(link)) = paragraph_children(&root).first() else {
            panic!("expected link for {source}");
        };
        assert_eq!(link.url, expected, "source={source}");
    }
}

#[test]
fn does_not_create_link_containing_link_nested_in_footnote() {
    let root = YozoraParser::default().parse(
        "[outer ^[[inner](u)]](v)",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let children = paragraph_children(&root);

    assert!(matches!(children.first(), Some(Node::Text(text)) if text.value == "[outer "));
    let Some(Node::Footnote(footnote)) = children.get(1) else {
        panic!("expected footnote");
    };
    assert!(matches!(
        footnote.children.first(),
        Some(Node::Link(link)) if link.url == "u"
    ));
    assert!(matches!(children.get(2), Some(Node::Text(text)) if text.value == "](v)"));
}
