use yozora_ast::Node;

pub enum NodeMatcher<'a> {
    All,
    Types(&'a [&'a str]),
    Predicate(&'a dyn Fn(&Node) -> bool),
}

impl NodeMatcher<'_> {
    pub fn matches(&self, node: &Node) -> bool {
        match self {
            Self::All => true,
            Self::Types(types) => types.iter().any(|node_type| *node_type == node.node_type()),
            Self::Predicate(predicate) => predicate(node),
        }
    }
}

pub fn create_node_matcher<'a>(types: Option<&'a [&'a str]>) -> NodeMatcher<'a> {
    match types {
        Some(types) => NodeMatcher::Types(types),
        None => NodeMatcher::All,
    }
}
