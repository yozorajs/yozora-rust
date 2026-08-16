use yozora_ast::{Node, Root};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;

fn parse_all(source: &str) -> [Root; 3] {
    let options = || ParseOptions {
        should_reserve_position: Some(false),
        ..ParseOptions::default()
    };

    [
        GfmParser::default().parse(source, Some(options())),
        GfmExParser::default().parse(source, Some(options())),
        YozoraParser::default().parse(source, Some(options())),
    ]
}

#[test]
fn parses_destination_and_title_after_definition_label_line() {
    for root in parse_all("[foo]:\n/url \"title\"") {
        let Some(Node::Definition(definition)) = root.children.first() else {
            panic!("expected definition");
        };
        assert_eq!(definition.identifier, "foo");
        assert_eq!(definition.label, "foo");
        assert_eq!(definition.url, "/url");
        assert_eq!(definition.title.as_deref(), Some("title"));
    }
}

#[test]
fn preserves_unicode_whitespace_when_normalizing_labels() {
    let nbsp_label = "a\u{00A0}b";
    let em_space_label = "a\u{2003}b";
    let source = format!(
        "[{nbsp_label}]: /nbsp\n[{em_space_label}]: /em-space\n[a b]: /ascii\n\n[ascii][a\tb] [nbsp][{nbsp_label}] [em][{em_space_label}]"
    );

    for root in parse_all(&source) {
        let identifiers: Vec<&str> = root
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Definition(definition) => Some(definition.identifier.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(identifiers, [nbsp_label, em_space_label, "a b"]);

        let Some(Node::Paragraph(paragraph)) = root.children.last() else {
            panic!("expected paragraph");
        };
        let references: Vec<&str> = paragraph
            .children
            .iter()
            .filter_map(|node| match node {
                Node::LinkReference(reference) => Some(reference.identifier.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(references, ["a b", nbsp_label, em_space_label]);
    }
}

#[test]
fn limits_definition_labels_to_999_characters() {
    let accepted = format!("[{}]: /url", "a".repeat(999));
    for root in parse_all(&accepted) {
        assert!(matches!(root.children.first(), Some(Node::Definition(_))));
    }

    for length in [1_000, 1_001] {
        let rejected = format!("[{}]: /url", "a".repeat(length));
        for root in parse_all(&rejected) {
            assert!(matches!(root.children.first(), Some(Node::Paragraph(_))));
        }
    }
}

#[test]
fn resolves_unicode_case_fold_equivalent_labels() {
    for source in [
        "[\u{1C80}]: /url\n\n[\u{0432}]",
        "[\u{0432}]: /url\n\n[\u{1C80}]",
    ] {
        for root in parse_all(source) {
            let Some(Node::Definition(definition)) = root.children.first() else {
                panic!("expected definition");
            };
            assert_eq!(definition.identifier, "\u{0432}");

            let Some(Node::Paragraph(paragraph)) = root.children.get(1) else {
                panic!("expected paragraph");
            };
            assert!(matches!(
                paragraph.children.first(),
                Some(Node::LinkReference(reference)) if reference.identifier == "\u{0432}"
            ));
        }
    }
}

#[test]
fn rejects_unbalanced_definition_destinations() {
    for source in ["[foo]: /url(foo", "[foo]: /url(foo\n"] {
        for root in parse_all(source) {
            assert_eq!(root.children.len(), 1);
            let Some(Node::Paragraph(paragraph)) = root.children.first() else {
                panic!("expected paragraph");
            };
            assert!(matches!(
                paragraph.children.as_slice(),
                [Node::Text(text)] if text.value == "[foo]: /url(foo"
            ));
        }
    }
}

#[test]
fn preserves_trailing_backslash_in_destination_at_eof() {
    for root in parse_all("[foo]: /url\\") {
        let Some(Node::Definition(definition)) = root.children.first() else {
            panic!("expected definition");
        };
        assert_eq!(definition.url, "/url%5C");
        assert_eq!(definition.title, None);
    }
}

#[test]
fn rejects_whitespace_escaped_in_definition_destination() {
    for root in parse_all("[x]: a\\ b\n\n[x]") {
        assert!(!root
            .children
            .iter()
            .any(|node| matches!(node, Node::Definition(_))));
        let Some(Node::Paragraph(paragraph)) = root.children.last() else {
            panic!("expected trailing paragraph");
        };
        assert!(matches!(
            paragraph.children.as_slice(),
            [Node::Text(text)] if text.value == "[x]"
        ));
    }
}

#[test]
fn validates_trailing_content_after_parenthesized_title() {
    for source in ["[foo]: /url (title)  ", "[foo]: /url (title)  \n"] {
        for root in parse_all(source) {
            let Some(Node::Definition(definition)) = root.children.first() else {
                panic!("expected definition");
            };
            assert_eq!(definition.url, "/url");
            assert_eq!(definition.title.as_deref(), Some("title"));
        }
    }

    for root in parse_all("[foo]: /url (title) x") {
        let Some(Node::Paragraph(paragraph)) = root.children.first() else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            paragraph.children.as_slice(),
            [Node::Text(text)] if text.value == "[foo]: /url (title) x"
        ));
    }
}
