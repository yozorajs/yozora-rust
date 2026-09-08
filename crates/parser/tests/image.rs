use std::sync::{Arc, Mutex};

use yozora_ast::Node;
use yozora_core_parser::ParseOptions;
use yozora_parser_gfm::GfmParser;

#[test]
fn image_omits_position_when_disabled() {
    let root = GfmParser::default().parse(
        "![alt](<https://example.com> \"title\")",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = root.children.first() else {
        panic!("expected paragraph");
    };
    let Some(Node::Image(image)) = paragraph.children.first() else {
        panic!("expected image");
    };
    assert_eq!(image.url, "https://example.com");
    assert_eq!(image.alt, "alt");
    assert_eq!(image.title.as_deref(), Some("title"));
    assert_eq!(image.position, None);
}

#[test]
fn empty_image_destinations_are_formatted() {
    for source in ["![x]()", "![x](<>)"] {
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
        let Some(Node::Paragraph(paragraph)) = root.children.first() else {
            panic!("expected paragraph");
        };
        let Some(Node::Image(image)) = paragraph.children.first() else {
            panic!("expected image");
        };
        assert_eq!(
            *formatted_urls.lock().expect("url recorder lock"),
            vec![String::new()]
        );
        assert_eq!(image.url, "formatted:");
    }
}

#[test]
fn formats_urls_in_depth_first_source_order_while_materializing_image_alt() {
    let urls = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&urls);
    let root = GfmParser::default().parse(
        "[![**A** ![B](inner \"I\")](outer \"O\")](link \"L\") ![C](sibling)",
        Some(ParseOptions {
            format_url: Some(Arc::new(move |url| {
                recorded.lock().unwrap().push(url.to_string());
                format!("formatted:{url}")
            })),
            ..ParseOptions::default()
        }),
    );
    assert_eq!(*urls.lock().unwrap(), ["link", "outer", "inner", "sibling"]);
    let Node::Paragraph(paragraph) = &root.children[0] else {
        panic!("expected paragraph")
    };
    let Node::Link(link) = &paragraph.children[0] else {
        panic!("expected link")
    };
    let Node::Image(image) = &link.children[0] else {
        panic!("expected image")
    };
    assert_eq!(image.alt, "A B");
    assert_eq!(image.url, "formatted:outer");
    assert_eq!(image.title.as_deref(), Some("O"));
}
