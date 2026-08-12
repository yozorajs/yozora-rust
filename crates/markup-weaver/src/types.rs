use std::sync::Arc;

use yozora_ast::{Node, Root};

pub type Escaper = Arc<dyn Fn(&str) -> String + Send + Sync + 'static>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeMarkup {
    pub opener: Option<String>,
    pub closer: Option<String>,
    pub indent: Option<String>,
    pub content: Option<String>,
    pub spread: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub enum Ancestor<'a> {
    Root(&'a Root),
    Node(&'a Node),
}

impl Ancestor<'_> {
    pub fn node_type(&self) -> &str {
        match self {
            Self::Root(root) => &root.node_type,
            Self::Node(node) => node.node_type(),
        }
    }

    pub fn children(&self) -> &[Node] {
        match self {
            Self::Root(root) => &root.children,
            Self::Node(node) => node.children().unwrap_or(&[]),
        }
    }
}

pub struct NodeMarkupWeaveContext<'a> {
    ancestors: Vec<Ancestor<'a>>,
    weave_inline_nodes: Box<dyn Fn(&'a [Node]) -> String + 'a>,
}

impl<'a> NodeMarkupWeaveContext<'a> {
    pub(crate) fn new<F>(ancestors: Vec<Ancestor<'a>>, weave_inline_nodes: F) -> Self
    where
        F: Fn(&'a [Node]) -> String + 'a,
    {
        Self {
            ancestors,
            weave_inline_nodes: Box::new(weave_inline_nodes),
        }
    }

    pub fn ancestors(&self) -> &[Ancestor<'a>] {
        &self.ancestors
    }

    pub fn weave_inline_nodes(&self, nodes: &'a [Node]) -> String {
        (self.weave_inline_nodes)(nodes)
    }
}

pub trait NodeWeaver: Send + Sync {
    fn node_types(&self) -> Vec<&str>;

    fn escape_content(&self) -> Option<Escaper> {
        None
    }

    fn is_block_level<'a>(
        &self,
        node: &'a Node,
        context: &NodeMarkupWeaveContext<'a>,
        child_index: usize,
    ) -> bool;

    fn weave<'a>(
        &self,
        node: &'a Node,
        context: &NodeMarkupWeaveContext<'a>,
        child_index: usize,
    ) -> NodeMarkup;
}

pub trait MarkupWeaverContract {
    fn use_weaver(&mut self, weaver: Box<dyn NodeWeaver>, force_replace: bool) -> &mut Self;
    fn unmount_weaver(&mut self, node_type: &str) -> &mut Self;
    fn weave(&self, ast: &Root) -> String;
}
