#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShallowNode<T> {
    One(T),
    Many(Vec<T>),
    None,
}

pub struct ShallowNodeCollector<T> {
    nodes: Vec<T>,
    next_nodes: Option<Vec<T>>,
}

impl<T> ShallowNodeCollector<T>
where
    T: Clone + PartialEq,
{
    pub fn add(&mut self, node: ShallowNode<T>, original_node: &T, original_index: usize) {
        if matches!(&node, ShallowNode::One(next_node) if next_node == original_node) {
            if let (Some(next_nodes), ShallowNode::One(node)) = (&mut self.next_nodes, node) {
                next_nodes.push(node);
            }
            return;
        }
        let next_nodes = self
            .next_nodes
            .get_or_insert_with(|| self.nodes[..original_index.min(self.nodes.len())].to_vec());
        match node {
            ShallowNode::One(node) => next_nodes.push(node),
            ShallowNode::Many(nodes) => next_nodes.extend(nodes),
            ShallowNode::None => {}
        }
    }

    pub fn collect(self) -> Vec<T> {
        self.next_nodes.unwrap_or(self.nodes)
    }
}

pub fn create_shallow_node_collector<T>(nodes: Vec<T>) -> ShallowNodeCollector<T> {
    ShallowNodeCollector {
        nodes,
        next_nodes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{create_shallow_node_collector, ShallowNode};

    #[test]
    fn shallow_collector_reuses_or_changes_sequence() {
        let nodes = vec![1, 2, 3];
        let mut unchanged = create_shallow_node_collector(nodes.clone());
        for (index, node) in nodes.iter().copied().enumerate() {
            unchanged.add(ShallowNode::One(node), &node, index);
        }
        assert_eq!(unchanged.collect(), nodes);
        let mut changed = create_shallow_node_collector(vec![1, 2, 3]);
        changed.add(ShallowNode::One(1), &1, 0);
        changed.add(ShallowNode::Many(vec![4, 5]), &2, 1);
        changed.add(ShallowNode::None, &3, 2);
        assert_eq!(changed.collect(), vec![1, 4, 5]);
    }
}
