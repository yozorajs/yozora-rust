use yozora_ast::Node;
use yozora_core_parser::ParseOptions;
use yozora_parser_gfm_ex::GfmExParser;

#[test]
fn recognizes_valid_extended_urls() {
    for source in ["ftp://example.com", "http://example.com/"] {
        assert_eq!(link_urls(source), vec![source.to_string()]);
    }
}

#[test]
fn rejects_underscore_in_final_domain_segments() {
    assert!(link_urls("http://foo_bar.com").is_empty());
}

#[test]
fn recognizes_valid_www_after_rejected_candidate_and_boundary() {
    for boundary in ['(', '*', '~'] {
        let source = format!("www.foo_bar.com{boundary}www.good.com");
        assert_eq!(link_urls(&source), vec!["http://www.good.com".to_string()]);
    }
}

#[test]
fn recognizes_url_after_underscore_boundary() {
    assert_eq!(
        link_urls("aa._https://example.com"),
        vec!["https://example.com".to_string()]
    );
    assert_eq!(
        link_urls("aa._www.example.com"),
        vec!["http://www.example.com".to_string()]
    );
}

#[test]
fn recognizes_extended_email_local_part_boundaries() {
    for source in [
        "+foo@bar.baz",
        ".foo@bar.baz",
        "-foo@bar.baz",
        "_foo@bar.baz",
        "foo@bar.baz",
    ] {
        assert_eq!(link_urls(source), vec![format!("mailto:{source}")]);
    }
}

#[test]
fn rejects_empty_email_domain_segments_and_trims_period() {
    assert!(link_urls("foo@bar..baz").is_empty());
    assert!(link_urls("foo@bar...").is_empty());
    assert_eq!(
        link_urls("foo@bar.baz."),
        vec!["mailto:foo@bar.baz".to_string()]
    );
}

#[test]
fn recognizes_extended_protocol_autolinks() {
    for source in [
        "mailto:foo@bar.baz",
        "xmpp:foo@bar.baz",
        "xmpp:foo@bar.baz/txt@bin.com",
    ] {
        assert_eq!(link_urls(source), vec![source.to_string()]);
    }

    assert_eq!(
        link_urls("prefix mailto:foo@bar.baz"),
        vec!["mailto:foo@bar.baz".to_string()]
    );
    assert_eq!(
        link_urls("xmpp:foo@bar.baz/txt/bin"),
        vec!["xmpp:foo@bar.baz/txt".to_string()]
    );
}

#[test]
fn explicit_link_overrides_protocol_like_label() {
    let ast = GfmExParser::default().parse(
        "[mailto:foo@bar.baz](https://example.com)",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph");
    };
    assert!(matches!(
        paragraph.children.as_slice(),
        [Node::Link(link)]
            if link.url == "https://example.com"
                && matches!(link.children.as_slice(), [Node::Text(text)] if text.value == "mailto:foo@bar.baz")
    ));
}

#[test]
fn rejects_email_suffix_inside_invalid_mailto() {
    for source in ["mailto:a.b-c_d@a.b-", "mailto:a.b-c_d@a.b_"] {
        let ast = GfmExParser::default().parse(
            source,
            Some(ParseOptions {
                should_reserve_position: Some(false),
                ..ParseOptions::default()
            }),
        );
        let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            paragraph.children.as_slice(),
            [Node::Text(text)] if text.value == source
        ));
    }
}

#[test]
fn keeps_protocol_trailing_punctuation_outside_link() {
    let ast = GfmExParser::default().parse(
        "mailto:a.b-c_d@a.b./",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph");
    };
    assert!(matches!(
        paragraph.children.as_slice(),
        [Node::Link(link), Node::Text(text)]
            if link.url == "mailto:a.b-c_d@a.b" && text.value == "./"
    ));

    let ast = GfmExParser::default().parse(
        "xmpp:foo@bar.baz/txt/bin",
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph");
    };
    assert!(matches!(
        paragraph.children.as_slice(),
        [Node::Link(link), Node::Text(text)]
            if link.url == "xmpp:foo@bar.baz/txt" && text.value == "/bin"
    ));
}

#[test]
fn large_rejected_candidates_remain_plain_text() {
    for source in [
        "_a".repeat(2_000),
        "_www.".repeat(1_000),
        "http://foo_bar.com(".repeat(1_000),
    ] {
        let ast = GfmExParser::default().parse(
            source.clone(),
            Some(ParseOptions {
                should_reserve_position: Some(false),
                ..ParseOptions::default()
            }),
        );
        let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            paragraph.children.as_slice(),
            [Node::Text(text)] if text.value == source
        ));
    }
}

fn link_urls(source: &str) -> Vec<String> {
    let ast = GfmExParser::default().parse(
        source,
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph");
    };
    paragraph
        .children
        .iter()
        .filter_map(|node| match node {
            Node::Link(link) => Some(link.url.clone()),
            _ => None,
        })
        .collect()
}
