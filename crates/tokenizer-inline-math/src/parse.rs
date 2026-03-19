use yozora_ast::{InlineMath, Node};
use yozora_character::{calc_string_from_node_points, is_space_like};
use yozora_core_tokenizer::{InlineToken, NodeInterval, ParseInlinePhaseApi};

#[derive(Debug, Clone)]
pub(crate) struct InlineMathTokenData {
    pub thickness: usize,
}

pub(crate) fn parse_inline_math_tokens(
    tokens: &[InlineToken],
    parse_api: &dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let node_points = parse_api.get_node_points();
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<InlineMathTokenData>() else {
            continue;
        };

        if token.start_index >= token.end_index || token.end_index > node_points.len() {
            continue;
        }

        let mut start_index = token.start_index.saturating_add(data.thickness);
        let mut end_index = token.end_index.saturating_sub(data.thickness);
        if start_index > end_index || end_index > node_points.len() {
            continue;
        }

        let mut is_all_space = true;
        for point in &node_points[start_index..end_index] {
            if is_space_like(point.code_point) {
                continue;
            }
            is_all_space = false;
            break;
        }

        if !is_all_space && start_index + 2 < end_index {
            let first_character = node_points[start_index].code_point;
            let last_character = node_points[end_index - 1].code_point;
            if is_space_like(first_character) && is_space_like(last_character) {
                start_index += 1;
                end_index -= 1;
            }
        }

        let value = calc_string_from_node_points(node_points, start_index, end_index, false)
            .replace('\n', " ");

        let position = if parse_api.should_reserve_position() {
            parse_api.calc_position(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            })
        } else {
            None
        };

        nodes.push(Node::InlineMath(InlineMath { position, value }));
    }

    nodes
}
