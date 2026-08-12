use yozora_ast::Node;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;

#[derive(Default)]
struct NodeStats {
    heading: usize,
    blockquote: usize,
    list: usize,
    table: usize,
    code: usize,
    math: usize,
    link: usize,
    image: usize,
    inline_code: usize,
    emphasis: usize,
    delete: usize,
    text: usize,
}

fn walk_nodes(nodes: &[Node], stats: &mut NodeStats) {
    for node in nodes {
        match node {
            Node::Admonition(x) => {
                walk_nodes(&x.title, stats);
                walk_nodes(&x.children, stats);
            }
            Node::Blockquote(x) => {
                stats.blockquote += 1;
                walk_nodes(&x.children, stats);
            }
            Node::Code(_) => {
                stats.code += 1;
            }
            Node::Delete(x) => {
                stats.delete += 1;
                walk_nodes(&x.children, stats);
            }
            Node::Emphasis(x) => {
                stats.emphasis += 1;
                walk_nodes(&x.children, stats);
            }
            Node::Footnote(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::FootnoteDefinition(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::Heading(x) => {
                stats.heading += 1;
                walk_nodes(&x.children, stats);
            }
            Node::Image(_) => {
                stats.image += 1;
            }
            Node::InlineCode(_) => {
                stats.inline_code += 1;
            }
            Node::InlineMath(_) => {}
            Node::Link(x) => {
                stats.link += 1;
                walk_nodes(&x.children, stats);
            }
            Node::LinkReference(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::List(x) => {
                stats.list += 1;
                walk_nodes(&x.children, stats);
            }
            Node::ListItem(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::Math(_) => {
                stats.math += 1;
            }
            Node::Paragraph(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::Strong(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::Table(x) => {
                stats.table += 1;
                walk_nodes(&x.children, stats);
            }
            Node::TableRow(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::TableCell(x) => {
                walk_nodes(&x.children, stats);
            }
            Node::Text(_) => {
                stats.text += 1;
            }
            Node::Break(_) => {}
            Node::Definition(_) => {}
            Node::EcmaImport(_) => {}
            Node::FootnoteReference(_) => {}
            Node::Frontmatter(_) => {}
            Node::Html(_) => {}
            Node::ImageReference(_) => {}
            Node::ThematicBreak(_) => {}
            Node::Custom(_) => {}
        }
    }
}

#[test]
fn parse_large_mixed_markdown_should_cover_core_nodes() {
    let input = include_str!("fixtures/large_mixed_1k.md");
    assert!(input.len() >= 1024);

    let parser = YozoraParser::default();
    let root = parser.parse(input, None);
    assert!(root.children.len() >= 8);

    let mut stats = NodeStats::default();
    walk_nodes(&root.children, &mut stats);

    assert!(stats.heading >= 2);
    assert!(stats.blockquote >= 1);
    assert!(stats.list >= 2);
    assert!(stats.table >= 1);
    assert!(stats.code >= 1);
    assert!(stats.math >= 1);
    assert!(stats.link >= 1);
    assert!(stats.image >= 1);
    assert!(stats.inline_code >= 1);
    assert!(stats.emphasis >= 1);
    assert!(stats.delete >= 1);
    assert!(stats.text >= 10);
}

#[test]
fn parse_large_plain_text_should_remain_single_paragraph() {
    let input = include_str!("fixtures/large_plain_1k.md");
    assert!(input.len() >= 1024);

    let parser = YozoraParser::default();
    let root = parser.parse(input, None);
    assert_eq!(root.children.len(), 1);

    let Node::Paragraph(paragraph) = &root.children[0] else {
        panic!("expected paragraph node");
    };
    assert_eq!(paragraph.children.len(), 1);

    let Node::Text(text) = &paragraph.children[0] else {
        panic!("expected text node");
    };
    assert!(text.value.len() >= 1024);
}

#[test]
fn parse_large_mixed_markdown_should_keep_root_position_when_enabled() {
    let input = include_str!("fixtures/large_mixed_1k.md");

    let parser = YozoraParser::default();
    let root = parser.parse(
        input,
        Some(ParseOptions {
            should_reserve_position: Some(true),
            ..ParseOptions::default()
        }),
    );

    assert!(root.position.is_some());
}
