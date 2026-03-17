use yozora_ast::{Node, Root};

use crate::mutate::{mutate_node, mutate_root};

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
