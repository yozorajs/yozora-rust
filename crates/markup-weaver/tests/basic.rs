use serde_json::{Map, Value};
use yozora_ast::{
    CustomNode, Link, Node, Paragraph, Root, Text, LINK_TYPE, PARAGRAPH_TYPE, TEXT_TYPE,
};
use yozora_markup_weaver::{
    DefaultMarkupWeaver, MarkupWeaverContract, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver,
};

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

fn paragraph(children: Vec<Node>) -> Node {
    Node::Paragraph(Paragraph {
        position: None,
        children,
    })
}
