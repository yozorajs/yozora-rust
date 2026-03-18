use std::sync::Arc;

use yozora_character::NodePoint;

#[derive(Debug, Clone)]
pub struct PhrasingContentLine {
    pub node_points: Arc<Vec<NodePoint>>,
    pub start_index: usize,
    pub end_index: usize,
    pub first_non_whitespace_index: usize,
    pub count_of_precede_spaces: usize,
}

impl PhrasingContentLine {
    pub fn whole(node_points: Arc<Vec<NodePoint>>) -> Self {
        let end_index = node_points.len();
        Self {
            node_points,
            start_index: 0,
            end_index,
            first_non_whitespace_index: 0,
            count_of_precede_spaces: 0,
        }
    }
}
