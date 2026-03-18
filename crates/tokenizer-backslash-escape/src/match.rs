use yozora_character::{calc_escaped_string_from_node_points, create_node_point_generator};

pub(crate) fn match_backslash_escaped_text(input: &str) -> Option<String> {
    if !input.contains('\\') && !input.contains('&') {
        return None;
    }

    let chunks = create_node_point_generator(input);
    let Some(points) = chunks.first() else {
        return None;
    };

    let decoded = calc_escaped_string_from_node_points(points, 0, points.len(), false);
    if decoded == input {
        return None;
    }

    Some(decoded)
}
