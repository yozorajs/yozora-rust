use std::future::Future;
use std::pin::Pin;

use yozora_ast::{Node, Root};

use crate::{mutate_node, mutate_root, NodeReplacement};

pub mod post_order;
pub mod pre_order;

pub use post_order::shallow_mutate_ast_in_postorder_async;
pub use pre_order::shallow_mutate_ast_in_preorder_async;

pub type NodeReplacementFuture<'a> = Pin<Box<dyn Future<Output = NodeReplacement> + 'a>>;

pub(crate) fn append_replacement(replacement: NodeReplacement, output: &mut Vec<Node>) {
    match replacement {
        NodeReplacement::One(node) => output.push(node),
        NodeReplacement::Many(nodes) => output.extend(nodes),
        NodeReplacement::Remove => {}
    }
}

pub async fn mutate_node_async<F>(node: &mut Node, visitor: &mut F)
where
    F: FnMut(&mut Node),
{
    mutate_node(node, visitor);
}

pub async fn mutate_root_async<F>(root: &mut Root, visitor: &mut F)
where
    F: FnMut(&mut Node),
{
    mutate_root(root, visitor);
}
