use yozora_ast::Node;

pub fn collect_texts(nodes: &[Node]) -> Vec<String> {
    let mut texts = Vec::new();
    let mut stack = vec![(nodes, 0usize)];
    while let Some((nodes, index)) = stack.last_mut() {
        if *index >= nodes.len() {
            stack.pop();
            continue;
        }
        let node = &nodes[*index];
        *index += 1;
        let text = match node {
            Node::Code(node) => Some(node.value.as_str()),
            Node::Frontmatter(node) => Some(node.value.as_str()),
            Node::Html(node) => Some(node.value.as_str()),
            Node::Image(node) => Some(node.alt.as_str()),
            Node::ImageReference(node) => Some(node.alt.as_str()),
            Node::InlineCode(node) => Some(node.value.as_str()),
            Node::InlineMath(node) => Some(node.value.as_str()),
            Node::Math(node) => Some(node.value.as_str()),
            Node::Text(node) => Some(node.value.as_str()),
            _ => None,
        };
        if let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) {
            texts.push(text.to_string());
        } else if let Some(children) = node.children().filter(|children| !children.is_empty()) {
            stack.push((children, 0));
        }
    }
    texts
}
