use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use yozora_ast::{Admonition, Footnote, Node, Paragraph, Point, Position, Root, Strong, Text};
use yozora_ast_util::{
    calc_excerpt_ast, calc_heading_toc, collect_nodes, default_url_resolver, remove_positions,
    replace_footnotes_in_references, search_node, shallow_clone_ast,
    shallow_mutate_ast_in_postorder, shallow_mutate_ast_in_postorder_async,
    shallow_mutate_ast_in_preorder, traverse_ast, NodeMatcher, NodeReplacement,
    NodeReplacementFuture, ParentRef,
};

fn text(value: &str) -> Node {
    Node::Text(Text {
        position: None,
        value: value.to_string(),
    })
}

fn paragraph(children: Vec<Node>) -> Node {
    Node::Paragraph(Paragraph {
        position: None,
        children,
    })
}

#[test]
fn preorder_replacement_skips_replaced_descendants() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![paragraph(vec![
            text("a"),
            Node::Strong(Strong {
                position: None,
                children: vec![text("b")],
            }),
        ])],
    };
    let mut visited = Vec::new();
    let next = shallow_mutate_ast_in_preorder(
        &root,
        NodeMatcher::Types(&["text", "strong"]),
        |node, _, _| {
            visited.push(node.node_type().to_string());
            if matches!(node, Node::Strong(_)) {
                NodeReplacement::One(text("S"))
            } else {
                NodeReplacement::One(node.clone())
            }
        },
    );

    assert_eq!(visited, ["text", "strong"]);
    let Node::Paragraph(paragraph) = &next.children[0] else {
        panic!("expected paragraph")
    };
    assert!(matches!(&paragraph.children[1], Node::Text(text) if text.value == "S"));
}

#[test]
fn postorder_replacement_visits_descendants_first() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![paragraph(vec![Node::Strong(Strong {
            position: None,
            children: vec![text("b")],
        })])],
    };
    let mut visited = Vec::new();
    let _ = shallow_mutate_ast_in_postorder(&root, NodeMatcher::All, |node, _, _| {
        visited.push(node.node_type().to_string());
        NodeReplacement::One(node.clone())
    });
    assert_eq!(visited, ["text", "strong", "paragraph"]);
}

#[test]
fn excerpt_uses_utf16_units_without_splitting_astral_character() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![paragraph(vec![text("a😀b")])],
    };
    let excerpt = calc_excerpt_ast(&root, 2);
    let Node::Paragraph(paragraph) = &excerpt.children[0] else {
        panic!("expected paragraph")
    };
    assert!(matches!(&paragraph.children[0], Node::Text(text) if text.value == "a"));
}

#[test]
fn remove_positions_covers_admonition_title_and_body() {
    let position = Position {
        start: Point {
            line: 1,
            column: 1,
            offset: Some(0),
        },
        end: Point {
            line: 1,
            column: 2,
            offset: Some(1),
        },
        indent: None,
    };
    let root = Root {
        node_type: "root".to_string(),
        position: Some(position.clone()),
        children: vec![Node::Admonition(Admonition {
            position: Some(position.clone()),
            keyword: "note".to_string(),
            title: vec![Node::Text(Text {
                position: Some(position.clone()),
                value: "title".to_string(),
            })],
            children: vec![paragraph(vec![text("body")])],
        })],
    };
    let next = remove_positions(&root);
    let Node::Admonition(admonition) = &next.children[0] else {
        panic!("expected admonition")
    };
    assert!(next.position.is_none());
    assert!(admonition.position.is_none());
    assert!(admonition.title[0].position().is_none());
}

#[test]
fn toc_avoids_suffix_collisions() {
    let mut root = Root {
        node_type: "root".to_string(),
        position: None,
        children: ["constructor", "constructor", "foo", "foo-2", "foo", "foo-2"]
            .into_iter()
            .map(|value| {
                Node::Heading(yozora_ast::Heading {
                    position: None,
                    identifier: None,
                    depth: 1,
                    children: vec![text(value)],
                })
            })
            .collect(),
    };
    let toc = calc_heading_toc(&mut root, "");
    assert_eq!(
        toc.children
            .iter()
            .map(|node| node.identifier.as_str())
            .collect::<Vec<_>>(),
        [
            "constructor",
            "constructor-2",
            "foo",
            "foo-2",
            "foo-3",
            "foo-2-2"
        ]
    );
}

#[test]
fn url_resolver_matches_directory_and_opaque_semantics() {
    let cases: &[(&[Option<&str>], &str)] = &[
        (
            &[Some("https://base.example/docs"), Some("guide")],
            "https://base.example/docs/guide",
        ),
        (&[Some("https://base.example/docs"), Some("/root")], "/root"),
        (&[Some("/a"), Some("./"), Some("./b")], "/a/b"),
        (&[Some("https://x/a"), Some("../../b")], "https://x/b"),
        (&[Some("a"), Some("../../b")], "../b"),
        (&[Some("https://x/a"), Some("#frag")], "https://x/a#frag"),
        (&[Some("https://x/a"), Some("?q=1")], "https://x/a?q=1"),
        (
            &[Some("https://x/a?old=1"), Some("#frag")],
            "https://x/a?old=1#frag",
        ),
        (
            &[Some("https://x/a?q=1#old"), Some("#new")],
            "https://x/a?q=1#new",
        ),
        (
            &[Some("https://x/a?old=1#old"), Some("?q=1")],
            "https://x/a?q=1",
        ),
        (&[Some("https://x/a?q=1#old"), Some("b")], "https://x/a/b"),
        (
            &[Some("https://x/a"), Some("?next=/b//c")],
            "https://x/a?next=/b//c",
        ),
        (
            &[Some("https://x/a"), Some("b?q=1#frag")],
            "https://x/a/b?q=1#frag",
        ),
        (
            &[Some("prefix"), Some("//cdn.example/a"), Some("../b")],
            "//cdn.example/b",
        ),
        (&[Some("https://x/a"), Some("./")], "https://x/a/"),
        (
            &[Some(
                "blob:https://example.com/550e8400-e29b-41d4-a716-446655440000",
            )],
            "blob:https://example.com/550e8400-e29b-41d4-a716-446655440000",
        ),
        (
            &[Some("blob:https://example.com/id?download=1#part")],
            "blob:https://example.com/id?download=1#part",
        ),
        (
            &[Some("prefix"), Some("data:text/plain,a/../b")],
            "data:text/plain,a/../b",
        ),
        (
            &[Some("mailto:user/../admin@example.com")],
            "mailto:user/../admin@example.com",
        ),
        (&[Some("urn:example:a/../b")], "urn:example:a/../b"),
        (&[Some("about:foo/../bar")], "about:foo/../bar"),
        (
            &[Some("javascript:render(\"/a/../b\")")],
            "javascript:render(\"/a/../b\")",
        ),
        (&[Some("pkg:docs/a/../b")], "pkg:docs/a/../b"),
        (&[Some("custom:/a/../b")], "custom:/a/../b"),
        (&[Some("custom://host/a/../b")], "custom://host/b"),
    ];
    for (pieces, expected) in cases {
        assert_eq!(default_url_resolver(pieces), *expected, "{pieces:?}");
    }
}

#[test]
fn footnotes_use_postorder_and_admonition_title_order() {
    let direct = Node::Footnote(Footnote {
        position: None,
        children: vec![text("direct")],
    });
    let nested = Node::Footnote(Footnote {
        position: None,
        children: vec![text("nested")],
    });
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![Node::Admonition(Admonition {
            position: None,
            keyword: "note".to_string(),
            title: vec![direct],
            children: vec![paragraph(vec![Node::Strong(Strong {
                position: None,
                children: vec![nested],
            })])],
        })],
    };
    let mut definitions = HashMap::new();
    let next = replace_footnotes_in_references(&root, &mut definitions, "footnote-");
    let Node::Admonition(admonition) = &next.children[0] else {
        panic!("expected admonition")
    };
    assert!(
        matches!(&admonition.title[0], Node::FootnoteReference(reference) if reference.identifier == "footnote-1")
    );
    let path = search_node(
        &next,
        |node, _, _| matches!(node, Node::FootnoteReference(reference) if reference.identifier == "footnote-2"),
    );
    assert!(path.is_some());
    assert_eq!(definitions.len(), 2);
}

#[test]
fn deep_ast_operations_are_stack_safe() {
    const DEPTH: usize = 20_000;
    let mut node = text("x");
    for _ in 0..DEPTH {
        node = paragraph(vec![node]);
    }
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![node],
    };

    let mut count = 0usize;
    traverse_ast(&root, NodeMatcher::All, |_, _, _| count += 1);
    assert_eq!(count, DEPTH + 1);
    assert_eq!(
        search_node(&root, |node, _, _| node.node_type() == "text")
            .expect("text should exist")
            .len(),
        DEPTH + 1
    );
    assert_eq!(collect_nodes(&root, NodeMatcher::Types(&["text"])).len(), 1);

    let cloned = shallow_clone_ast(&root, |_, _, _| false);
    let excerpt = calc_excerpt_ast(&root, 1);
    let without_positions = remove_positions(&root);
    let mutated =
        shallow_mutate_ast_in_preorder(&root, NodeMatcher::Types(&["text"]), |node, _, _| {
            NodeReplacement::One(node.clone())
        });

    std::mem::forget(root);
    std::mem::forget(cloned);
    std::mem::forget(excerpt);
    std::mem::forget(without_positions);
    std::mem::forget(mutated);
}

#[test]
fn async_postorder_awaits_replacements() {
    fn replace<'a>(node: &'a Node, _: ParentRef<'a>, _: usize) -> NodeReplacementFuture<'a> {
        Box::pin(async move {
            match node {
                Node::Text(text) if text.value == "drop" => NodeReplacement::Remove,
                Node::Text(text) => NodeReplacement::One(super_text(&text.value.to_uppercase())),
                _ => NodeReplacement::One(node.clone()),
            }
        })
    }

    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![paragraph(vec![text("keep"), text("drop")])],
    };
    let next = block_on(shallow_mutate_ast_in_postorder_async(
        &root,
        NodeMatcher::Types(&["text"]),
        replace,
    ));
    let Node::Paragraph(paragraph) = &next.children[0] else {
        panic!("expected paragraph")
    };
    assert_eq!(paragraph.children.len(), 1);
    assert!(matches!(&paragraph.children[0], Node::Text(text) if text.value == "KEEP"));
}

fn super_text(value: &str) -> Node {
    text(value)
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Pin::from(Box::new(future));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
