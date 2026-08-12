use yozora_ast::Point;
use yozora_character::{NodePoint, VirtualCodePoint};

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
    let source_width = point
        .source_width
        .unwrap_or(if point.code_point > 0xffff { 2 } else { 1 });

    if point.code_point == VirtualCodePoint::LineEnd as i32 {
        return Point {
            line: point.line + 1,
            column: 1,
            offset: Some(point.offset + source_width),
        };
    }

    Point {
        line: point.line,
        column: point.column + source_width,
        offset: Some(point.offset + source_width),
    }
}
