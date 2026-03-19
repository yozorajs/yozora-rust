use yozora_ast::Point;
use yozora_character::NodePoint;

/// Resolve start point from a node-point list.
pub fn calc_start_point(node_points: &[NodePoint], index: usize) -> Point {
    let point = node_points[index];
    Point {
        line: point.line,
        column: point.column,
        offset: Some(point.offset),
    }
}

/// Resolve end point from a node-point list.
pub fn calc_end_point(node_points: &[NodePoint], index: usize) -> Point {
    let point = node_points[index];
    Point {
        line: point.line,
        column: point.column + 1,
        offset: Some(point.offset + 1),
    }
}
