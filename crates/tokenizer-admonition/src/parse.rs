use yozora_ast::{Admonition, Node, Point, Position, Text};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::AdmonitionBlockToken;

pub(crate) fn parse_admonition_block(
    lines: &[&str],
    position: Option<yozora_ast::Position>,
    token: AdmonitionBlockToken,
) -> BlockTokenizeResult {
    let first = lines[0];
    let line_starts = position
        .as_ref()
        .map(|p| calc_line_start_points(lines, p.start));

    let title_nodes = if token.title.is_empty() {
        Vec::new()
    } else {
        let title_position = line_starts.as_ref().and_then(|points| {
            let start = *points.first()?;
            let start_index = first.find(&token.title)?;
            calc_position_from_interval(start, first, start_index, start_index + token.title.len())
        });

        vec![Node::Text(Text {
            position: title_position,
            value: token.title.clone(),
        })]
    };

    let children = if token.body_lines.is_empty() {
        Vec::new()
    } else {
        let body_value = token.body_lines.join("\n");
        let body_position = line_starts.as_ref().and_then(|points| {
            let start = *points.get(1)?;
            calc_position_from_text(start, &body_value)
        });

        vec![Node::Text(Text {
            position: body_position,
            value: body_value,
        })]
    };

    let node_position = line_starts.as_ref().and_then(|points| {
        let start = *points.first()?;
        let last_line = *lines.get(token.consumed_lines.saturating_sub(1))?;
        let last_start = *points.get(token.consumed_lines.saturating_sub(1))?;
        let end = advance_point_by_text(last_start, last_line);
        Some(Position {
            start,
            end,
            indent: None,
        })
    });

    BlockTokenizeResult {
        node: Node::Admonition(Admonition {
            position: node_position,
            keyword: token.keyword,
            title: title_nodes,
            children,
        }),
        consumed_lines: token.consumed_lines,
    }
}

fn calc_position_from_interval(
    line_start: Point,
    line: &str,
    start_index: usize,
    end_index: usize,
) -> Option<Position> {
    if start_index > end_index || end_index > line.len() {
        return None;
    }
    if !line.is_char_boundary(start_index) || !line.is_char_boundary(end_index) {
        return None;
    }

    let start = advance_point_by_text(line_start, &line[..start_index]);
    let end = advance_point_by_text(start, &line[start_index..end_index]);
    Some(Position {
        start,
        end,
        indent: None,
    })
}

fn calc_position_from_text(start: Point, text: &str) -> Option<Position> {
    let end = advance_point_by_text(start, text);
    Some(Position {
        start,
        end,
        indent: None,
    })
}

fn calc_line_start_points(lines: &[&str], start: Point) -> Vec<Point> {
    let mut points = Vec::with_capacity(lines.len());
    let mut current = start;

    for (idx, line) in lines.iter().enumerate() {
        points.push(current);
        if idx + 1 >= lines.len() {
            continue;
        }

        let mut next = advance_point_by_text(current, line);
        if let Some(offset) = next.offset.as_mut() {
            *offset += 1;
        }
        next.line += 1;
        next.column = 1;
        current = next;
    }

    points
}

fn advance_point_by_text(mut point: Point, text: &str) -> Point {
    for ch in text.chars() {
        if let Some(offset) = point.offset.as_mut() {
            *offset += ch.len_utf8();
        }

        if ch == '\n' {
            point.line += 1;
            point.column = 1;
        } else {
            point.column += 1;
        }
    }

    point
}
