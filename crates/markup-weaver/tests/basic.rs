use serde_json::{Map, Value};
use yozora_ast::{
    Break, CustomNode, Emphasis, Link, Node, Paragraph, Root, Text, LINK_TYPE, PARAGRAPH_TYPE,
    TEXT_TYPE,
};
use yozora_core_parser::ParseOptions;
use yozora_markup_weaver::{
    DefaultMarkupWeaver, MarkupWeaverContract, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver,
};
use yozora_parser::YozoraParser;

#[test]
fn weaves_protocol_autolink() {
    let url = "mailto:foo@bar.baz";
    let ast = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![Node::Paragraph(Paragraph {
            position: None,
            children: vec![Node::Link(Link {
                position: None,
                url: url.to_string(),
                title: None,
                children: vec![Node::Text(Text {
                    position: None,
                    value: url.to_string(),
                })],
            })],
        })],
    };

    assert_eq!(DefaultMarkupWeaver::default().weave(&ast), url);
    let _ = (LINK_TYPE, PARAGRAPH_TYPE, TEXT_TYPE);
}

struct MentionWeaver;

impl NodeWeaver for MentionWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec!["mention"]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        false
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::Custom(node) = node else {
            unreachable!()
        };
        NodeMarkup {
            opener: Some(format!(
                "@{}",
                node.data
                    .get("value")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            )),
            ..NodeMarkup::default()
        }
    }
}

#[test]
fn supports_custom_node_types() {
    let mut data = Map::new();
    data.insert(
        "value".to_string(),
        Value::String("guanghechen".to_string()),
    );
    let ast = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![paragraph(vec![Node::Custom(CustomNode {
            node_type: "mention".to_string(),
            position: None,
            children: None,
            data,
        })])],
    };
    let mut weaver = DefaultMarkupWeaver::default();
    weaver.use_weaver(Box::new(MentionWeaver), false);
    assert_eq!(weaver.weave(&ast), "@guanghechen");
}

#[test]
fn weaves_hard_break_with_one_line_ending() {
    let ast = root(vec![paragraph(vec![
        text("foo"),
        Node::Break(Break { position: None }),
        text("bar"),
    ])]);
    let markup = DefaultMarkupWeaver::default().weave(&ast);
    assert_eq!(markup, "foo\\\nbar");
    assert_eq!(parse_without_positions(&markup), ast);

    let legacy = root(vec![paragraph(vec![
        text("foo"),
        Node::Break(Break { position: None }),
        text("\nbar"),
    ])]);
    assert_eq!(DefaultMarkupWeaver::default().weave(&legacy), "foo\\\nbar");
}

#[test]
fn escapes_block_markers_after_hard_break() {
    for tail in ["- bar", "* bar", "+ bar", "---", "___", "***"] {
        let ast = root(vec![paragraph(vec![
            text("foo"),
            Node::Break(Break { position: None }),
            text(tail),
        ])]);
        let markup = DefaultMarkupWeaver::default().weave(&ast);
        assert_eq!(markup, format!("foo\\\n\\{tail}"));
        assert_eq!(parse_without_positions(&markup), ast);
    }
}

#[test]
fn preserves_literal_backslashes_in_text() {
    let ast = root(vec![paragraph(vec![text(r"\]\*\a")])]);
    let markup = DefaultMarkupWeaver::default().weave(&ast);
    assert_eq!(markup, "\\\\]\\\\*\\a");
    assert_eq!(parse_without_positions(&markup), ast);
}

#[test]
fn preserves_backslash_before_nested_inline_delimiter() {
    let ast = root(vec![paragraph(vec![Node::Link(Link {
        position: None,
        url: "/target".to_string(),
        title: None,
        children: vec![Node::Emphasis(Emphasis {
            position: None,
            children: vec![text(r"\*")],
        })],
    })])]);
    let markup = DefaultMarkupWeaver::default().weave(&ast);
    assert_eq!(parse_without_positions(&markup), ast);
}

#[test]
fn keeps_outer_escaper_active_after_nested_same_type() {
    let ast = root(vec![paragraph(vec![Node::Emphasis(Emphasis {
        position: None,
        children: vec![
            text("before "),
            Node::Emphasis(Emphasis {
                position: None,
                children: vec![text("*inner*")],
            }),
            text("*x*"),
        ],
    })])]);
    let markup = DefaultMarkupWeaver::default().weave(&ast);
    assert_eq!(markup, "*before _\\*inner\\*_\\*x\\**");
    assert_eq!(parse_without_positions(&markup), ast);
}

#[test]
fn keeps_explicit_protocol_link_when_label_differs() {
    let ast = root(vec![paragraph(vec![Node::Link(Link {
        position: None,
        url: "mailto:foo@bar.baz".to_string(),
        title: None,
        children: vec![text("email")],
    })])]);
    assert_eq!(
        DefaultMarkupWeaver::default().weave(&ast),
        "[email](mailto:foo@bar.baz)"
    );
}

fn paragraph(children: Vec<Node>) -> Node {
    Node::Paragraph(Paragraph {
        position: None,
        children,
    })
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        position: None,
        value: value.to_string(),
    })
}

fn root(children: Vec<Node>) -> Root {
    Root {
        node_type: "root".to_string(),
        position: None,
        children,
    }
}

fn parse_without_positions(markup: &str) -> Root {
    YozoraParser::default().parse(
        markup,
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    )
}
