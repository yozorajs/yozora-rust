use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use serde_json::{json, Map, Value};
use yozora_ast::{
    Admonition, CustomNode, Definition, Footnote, FootnoteDefinition, Html, Image, ImageReference,
    List, ListItem, Node, Paragraph, Point, Position, ReferenceType, Root, Strong, Text,
};
use yozora_ast_util::mutate::{create_postorder_mutation, create_preorder_mutation, MutationStep};
use yozora_ast_util::{
    calc_definition_map, calc_excerpt_ast, calc_footnote_definition_map, calc_heading_toc,
    collect_nodes, default_url_resolver, get_excerpt_ast, remove_positions,
    replace_footnotes_in_references, resolve_urls_for_ast, search_node, shallow_clone_ast,
    shallow_mutate_ast_in_postorder, shallow_mutate_ast_in_postorder_async,
    shallow_mutate_ast_in_preorder, shallow_mutate_ast_in_preorder_async, traverse_ast,
    NodeMatcher, NodeReplacement, NodeReplacementFuture, ParentRef,
    DEFAULT_FOOTNOTE_IDENTIFIER_PREFIX,
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
fn mutation_factories_resume_with_replacements() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![text("a")],
    };
    let mut preorder = create_preorder_mutation(&root, NodeMatcher::All);
    let MutationStep::Request(request) = preorder.next(None) else {
        panic!("expected preorder request");
    };
    assert!(matches!(request.node, Node::Text(node) if node.value == "a"));
    let MutationStep::Complete(root) = preorder.next(Some(NodeReplacement::One(text("b")))) else {
        panic!("expected preorder completion");
    };
    assert!(matches!(root.children.as_slice(), [Node::Text(node)] if node.value == "b"));
}

#[test]
fn postorder_factory_yields_transformed_parent() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![Node::Strong(Strong {
            position: None,
            children: vec![text("a")],
        })],
    };
    let mut mutation = create_postorder_mutation(&root, NodeMatcher::All);
    let MutationStep::Request(request) = mutation.next(None) else {
        panic!("expected text request");
    };
    assert!(matches!(request.node, Node::Text(node) if node.value == "a"));

    let MutationStep::Request(request) = mutation.next(Some(NodeReplacement::One(text("b"))))
    else {
        panic!("expected transformed strong request");
    };
    assert!(matches!(
        request.node,
        Node::Strong(node)
            if matches!(node.children.as_slice(), [Node::Text(text)] if text.value == "b")
    ));
    let replacement = NodeReplacement::One(request.node.clone());
    let MutationStep::Complete(root) = mutation.next(Some(replacement)) else {
        panic!("expected postorder completion");
    };
    assert!(matches!(
        root.children.as_slice(),
        [Node::Strong(node)]
            if matches!(node.children.as_slice(), [Node::Text(text)] if text.value == "b")
    ));
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
fn excerpt_truncates_nested_literals_and_zero_limit() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![paragraph(vec![
            text("ab"),
            Node::Emphasis(yozora_ast::Emphasis {
                position: None,
                children: vec![text("cdefgh")],
            }),
            text("ignored"),
        ])],
    };

    let excerpt = calc_excerpt_ast(&root, 5);
    assert!(matches!(
        excerpt.children.as_slice(),
        [Node::Paragraph(paragraph)]
            if matches!(
                paragraph.children.as_slice(),
                [Node::Text(first), Node::Emphasis(emphasis)]
                    if first.value == "ab"
                        && matches!(emphasis.children.as_slice(), [Node::Text(text)] if text.value == "cde")
            )
    ));
    assert!(calc_excerpt_ast(&root, 0).children.is_empty());
    assert!(matches!(
        &root.children[0],
        Node::Paragraph(paragraph)
            if matches!(&paragraph.children[1], Node::Emphasis(emphasis)
                if matches!(emphasis.children.as_slice(), [Node::Text(text)] if text.value == "cdefgh"))
    ));
}

#[test]
fn excerpt_separator_stops_at_root_or_nested_literal() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![
            paragraph(vec![text("before")]),
            Node::Html(Html {
                position: None,
                value: "<!-- more -->".to_string(),
            }),
            paragraph(vec![text("after")]),
        ],
    };
    let excerpt = get_excerpt_ast(&root, 100, Some("<!-- more -->"));
    assert!(matches!(
        excerpt.children.as_slice(),
        [Node::Paragraph(paragraph)]
            if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "before")
    ));
    assert_eq!(root.children.len(), 3);

    let nested = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![
            paragraph(vec![text("before"), text("<!-- more -->"), text("after")]),
            paragraph(vec![text("later")]),
        ],
    };
    let excerpt = get_excerpt_ast(&nested, 100, Some("<!-- more -->"));
    assert!(matches!(
        excerpt.children.as_slice(),
        [Node::Paragraph(paragraph)]
            if matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == "before")
    ));
    assert_eq!(nested.children.len(), 2);
    assert_eq!(get_excerpt_ast(&nested, 100, Some("   ")), nested);
}

#[test]
fn definition_map_appends_only_missing_presets() {
    let definition = Definition {
        position: None,
        identifier: "constructor".to_string(),
        label: "constructor".to_string(),
        url: "/definition".to_string(),
        title: None,
    };
    let preset = Definition {
        position: None,
        identifier: "__proto__".to_string(),
        label: "__proto__".to_string(),
        url: "/preset".to_string(),
        title: None,
    };
    let duplicate = Definition {
        url: "/duplicate".to_string(),
        ..definition.clone()
    };
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![Node::Definition(definition.clone())],
    };

    let result = calc_definition_map(&root, &[preset.clone(), duplicate]);
    assert_eq!(result.root.children.len(), 2);
    assert_eq!(result.definition_map["constructor"].url, "/definition");
    assert_eq!(result.definition_map["__proto__"].url, "/preset");
    assert_eq!(root.children.len(), 1);
}

#[test]
fn footnote_definition_map_appends_missing_presets() {
    let definition = FootnoteDefinition {
        position: None,
        identifier: "constructor".to_string(),
        label: "constructor".to_string(),
        children: Vec::new(),
    };
    let preset = FootnoteDefinition {
        position: None,
        identifier: "__proto__".to_string(),
        label: "__proto__".to_string(),
        children: Vec::new(),
    };
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![Node::FootnoteDefinition(definition)],
    };

    let result = calc_footnote_definition_map(
        &root,
        std::slice::from_ref(&preset),
        false,
        DEFAULT_FOOTNOTE_IDENTIFIER_PREFIX,
    );
    assert_eq!(result.root.children.len(), 2);
    assert!(result.footnote_definition_map.contains_key("constructor"));
    assert!(result.footnote_definition_map.contains_key("__proto__"));
    assert_eq!(root.children.len(), 1);
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
fn remove_positions_preserves_metadata_and_cleans_custom_node_arrays() {
    let position = json!({
        "start": { "offset": 0, "line": 1, "column": 1 },
        "end": { "offset": 1, "line": 1, "column": 2 }
    });
    let mut data = Map::new();
    data.insert("tags".to_string(), json!(["alpha", "beta"]));
    data.insert(
        "title".to_string(),
        json!([{
            "type": "text",
            "value": "title",
            "position": position
        }]),
    );
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![Node::Custom(CustomNode {
            node_type: "custom".to_string(),
            position: Some(Position {
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
            }),
            children: None,
            data,
        })],
    };

    let next = remove_positions(&root);
    let Node::Custom(custom) = &next.children[0] else {
        panic!("expected custom node");
    };
    assert_eq!(custom.position, None);
    assert_eq!(custom.data["tags"], json!(["alpha", "beta"]));
    assert_eq!(
        custom.data["title"],
        json!([{ "type": "text", "value": "title" }])
    );

    let Node::Custom(original) = &root.children[0] else {
        panic!("expected original custom node");
    };
    assert!(matches!(
        &original.data["title"],
        Value::Array(items)
            if items[0].get("position").is_some()
    ));
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
fn search_returns_preorder_paths() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![
            text("first"),
            paragraph(vec![
                text("before"),
                Node::Strong(Strong {
                    position: None,
                    children: vec![text("bar")],
                }),
            ]),
        ],
    };

    assert_eq!(search_node(&root, |_, _, _| true), Some(vec![0]));
    assert_eq!(
        search_node(&root, |node, _, _| {
            matches!(node, Node::Text(text) if text.value == "bar")
        }),
        Some(vec![1, 1, 0])
    );
    assert_eq!(
        search_node(&root, |node, _, _| {
            matches!(node, Node::Text(text) if text.value == "missing")
        }),
        None
    );
}

#[test]
fn shallow_clone_stops_entire_traversal_at_nested_match() {
    fn custom(node_type: &str, children: Option<Vec<Node>>) -> Node {
        Node::Custom(CustomNode {
            node_type: node_type.to_string(),
            position: None,
            children,
            data: Map::new(),
        })
    }

    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![
            custom(
                "parent",
                Some(vec![custom("before", None), custom("stop", None)]),
            ),
            custom("later", None),
        ],
    };
    let mut visited = Vec::new();
    let cloned = shallow_clone_ast(&root, |node, _, _| {
        visited.push(node.node_type().to_string());
        node.node_type() == "stop"
    });

    assert_eq!(visited, ["parent", "before", "stop"]);
    assert_eq!(cloned.children.len(), 1);
    let Node::Custom(parent) = &cloned.children[0] else {
        panic!("expected parent");
    };
    assert!(matches!(
        parent.children.as_deref(),
        Some([Node::Custom(before)]) if before.node_type == "before"
    ));
}

#[test]
fn collect_nodes_respects_matcher_and_preorder() {
    let root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![
            paragraph(vec![text("a"), text("b")]),
            Node::List(List {
                position: None,
                ordered: false,
                order_type: None,
                start: None,
                marker: b'-' as u32,
                spread: false,
                children: vec![Node::ListItem(ListItem {
                    position: None,
                    status: None,
                    children: vec![paragraph(vec![text("c")])],
                })],
            }),
        ],
    };

    let texts = collect_nodes(&root, NodeMatcher::Types(&["text"]));
    assert_eq!(
        texts
            .iter()
            .filter_map(|node| match node {
                Node::Text(text) => Some(text.value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );

    let parents = collect_nodes(&root, NodeMatcher::Types(&["paragraph", "list"]));
    assert_eq!(
        parents
            .iter()
            .map(|node| node.node_type())
            .collect::<Vec<_>>(),
        ["paragraph", "list"]
    );
}

#[test]
fn toc_scales_for_many_duplicate_headings() {
    const SIZE: usize = 10_000;
    let mut root = Root {
        node_type: "root".to_string(),
        position: None,
        children: (0..SIZE)
            .map(|_| {
                Node::Heading(yozora_ast::Heading {
                    position: None,
                    identifier: None,
                    depth: 1,
                    children: vec![text("title")],
                })
            })
            .collect(),
    };

    let toc = calc_heading_toc(&mut root, "");
    assert_eq!(toc.children.len(), SIZE);
    assert_eq!(
        toc.children.last().map(|node| node.identifier.as_str()),
        Some("title-10000")
    );
    assert_eq!(
        toc.children
            .iter()
            .map(|node| node.identifier.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        SIZE
    );
}

#[test]
fn toc_identifiers_include_image_alt_text() {
    let mut root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![
            Node::Heading(yozora_ast::Heading {
                position: None,
                identifier: None,
                depth: 1,
                children: vec![Node::Image(Image {
                    position: None,
                    url: "/image".to_string(),
                    title: None,
                    alt: "Alpha beta".to_string(),
                })],
            }),
            Node::Heading(yozora_ast::Heading {
                position: None,
                identifier: None,
                depth: 1,
                children: vec![Node::ImageReference(ImageReference {
                    position: None,
                    identifier: "image".to_string(),
                    label: "image".to_string(),
                    reference_type: ReferenceType::Full,
                    alt: "Gamma delta".to_string(),
                })],
            }),
            Node::Heading(yozora_ast::Heading {
                position: None,
                identifier: None,
                depth: 1,
                children: vec![
                    text("Before"),
                    Node::Image(Image {
                        position: None,
                        url: "/image".to_string(),
                        title: None,
                        alt: "Alpha".to_string(),
                    }),
                    text("After"),
                ],
            }),
        ],
    };

    let toc = calc_heading_toc(&mut root, "");
    assert_eq!(
        toc.children
            .iter()
            .map(|node| node.identifier.as_str())
            .collect::<Vec<_>>(),
        ["alpha-beta", "gamma-delta", "before-alpha-after"]
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
fn resolve_urls_uses_default_matcher_and_resolver() {
    let mut root = Root {
        node_type: "root".to_string(),
        position: None,
        children: vec![Node::Link(yozora_ast::Link {
            position: None,
            url: "docs/../guide".to_string(),
            title: None,
            children: Vec::new(),
        })],
    };

    resolve_urls_for_ast(&mut root);

    let Node::Link(link) = &root.children[0] else {
        panic!("expected link");
    };
    assert_eq!(link.url, "guide");
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

#[test]
fn async_preorder_replaces_parent_before_descendants() {
    fn replace<'a>(node: &'a Node, _: ParentRef<'a>, _: usize) -> NodeReplacementFuture<'a> {
        Box::pin(async move {
            match node {
                Node::Strong(_) => NodeReplacement::One(text("S")),
                _ => NodeReplacement::One(node.clone()),
            }
        })
    }

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
    let next = block_on(shallow_mutate_ast_in_preorder_async(
        &root,
        NodeMatcher::Types(&["text", "strong"]),
        replace,
    ));
    let Node::Paragraph(paragraph) = &next.children[0] else {
        panic!("expected paragraph");
    };
    assert!(matches!(
        paragraph.children.as_slice(),
        [Node::Text(first), Node::Text(second)] if first.value == "a" && second.value == "S"
    ));
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
