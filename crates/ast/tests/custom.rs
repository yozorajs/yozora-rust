use serde_json::json;
use yozora_ast::{CustomNode, Node};

#[test]
fn custom_node_round_trips_dynamic_type_and_fields() {
    let value = json!({
        "type": "mention",
        "value": "guanghechen",
        "children": [{ "type": "text", "value": "child" }]
    });
    let node: Node = serde_json::from_value(value.clone()).unwrap();
    let Node::Custom(CustomNode {
        node_type,
        children,
        data,
        ..
    }) = &node
    else {
        panic!("expected custom node")
    };
    assert_eq!(node_type, "mention");
    assert_eq!(children.as_ref().map(Vec::len), Some(1));
    assert_eq!(data.get("value"), Some(&json!("guanghechen")));
    assert_eq!(serde_json::to_value(node).unwrap(), value);
}

#[test]
fn known_node_serialization_is_unchanged() {
    let value = json!({ "type": "text", "value": "hello" });
    let node: Node = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(node).unwrap(), value);
}
