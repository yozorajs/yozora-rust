use yozora_ast::{Node, Root};
use yozora_core_parser::ParseOptions;
use yozora_core_tokenizer::{AnyTokenizer, Tokenizer};
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_tokenizer_definition::DefinitionTokenizer;
use yozora_tokenizer_footnote::FootnoteTokenizer;
use yozora_tokenizer_footnote_definition::FootnoteDefinitionTokenizer;
use yozora_tokenizer_footnote_reference::FootnoteReferenceTokenizer;

fn options() -> Option<ParseOptions> {
    Some(ParseOptions {
        should_reserve_position: Some(false),
        ..ParseOptions::default()
    })
}

fn gfm_with_footnotes() -> GfmParser {
    let mut parser = GfmParser::default();
    parser.use_tokenizer(
        AnyTokenizer::Block(Box::new(FootnoteDefinitionTokenizer::default())),
        Some(DefinitionTokenizer::default().name()),
    );
    parser.use_tokenizer(
        AnyTokenizer::Inline(Box::new(FootnoteTokenizer::default())),
        None,
    );
    parser.use_tokenizer(
        AnyTokenizer::Inline(Box::new(FootnoteReferenceTokenizer::default())),
        None,
    );
    parser
}

fn paragraph_children(root: &Root) -> &[Node] {
    let Some(Node::Paragraph(paragraph)) = root.children.first() else {
        panic!("expected paragraph");
    };
    &paragraph.children
}

#[test]
fn recognizes_definition_after_partial_tab_indentation() {
    let root = gfm_with_footnotes().parse("1234. foo\n\n\t  \t[^1]: /url", options());
    let Some(Node::List(list)) = root.children.first() else {
        panic!("expected list");
    };
    let Some(Node::ListItem(item)) = list.children.first() else {
        panic!("expected list item");
    };
    assert!(matches!(
        item.children.get(1),
        Some(Node::FootnoteDefinition(_))
    ));
}

#[test]
fn recognizes_escaped_and_maximum_length_labels() {
    let escaped = YozoraParser::default().parse("[^\\]]: note\n\n[^\\]]", options());
    let Some(Node::FootnoteDefinition(definition)) = escaped.children.first() else {
        panic!("expected definition");
    };
    assert_eq!(definition.label, r"\]");
    let Some(Node::Paragraph(paragraph)) = escaped.children.get(1) else {
        panic!("expected paragraph");
    };
    assert!(matches!(
        paragraph.children.first(),
        Some(Node::FootnoteReference(reference)) if reference.label == r"\]"
    ));

    let maximum_label = "a".repeat(999);
    let maximum = YozoraParser::default().parse(format!("[^{maximum_label}]: note"), options());
    assert!(matches!(
        maximum.children.first(),
        Some(Node::FootnoteDefinition(definition)) if definition.label == maximum_label
    ));
}

#[test]
fn does_not_nest_inline_footnotes() {
    let root = YozoraParser::default().parse("^[outer ^[inner]]", options());
    let children = paragraph_children(&root);
    assert!(matches!(children.first(), Some(Node::Text(text)) if text.value == "^[outer "));
    assert!(matches!(
        children.get(1),
        Some(Node::Footnote(footnote))
            if matches!(footnote.children.as_slice(), [Node::Text(text)] if text.value == "inner")
    ));
    assert!(matches!(children.get(2), Some(Node::Text(text)) if text.value == "]"));
}

#[test]
fn does_not_nest_inline_footnotes_through_links() {
    let root = YozoraParser::default().parse("^[outer [label ^[inner]](/url)]", options());
    let children = paragraph_children(&root);
    assert!(matches!(children.first(), Some(Node::Text(text)) if text.value == "^[outer "));
    let Some(Node::Link(link)) = children.get(1) else {
        panic!("expected link");
    };
    assert_eq!(link.url, "/url");
    assert!(matches!(link.children.first(), Some(Node::Text(text)) if text.value == "label "));
    assert!(matches!(link.children.get(1), Some(Node::Footnote(_))));
    assert!(matches!(children.get(2), Some(Node::Text(text)) if text.value == "]"));
}
