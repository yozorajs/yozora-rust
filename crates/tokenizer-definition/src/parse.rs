use yozora_ast::{Definition, Node};
use yozora_character::{calc_escaped_string_from_node_points, AsciiCodePoint};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

use crate::r#match::{calc_effective_position, resolve_label_and_identifier};
use crate::types::DefinitionTokenData;

pub(crate) fn parse_definition_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<DefinitionTokenData>() else {
            continue;
        };

        let Some((label, identifier)) = resolve_label_and_identifier(&data.label.node_points)
        else {
            continue;
        };

        let Some(destination_state) = &data.destination else {
            continue;
        };
        if !destination_state.saturated {
            continue;
        }

        let destination_points = &destination_state.node_points;
        let destination = if destination_points
            .first()
            .is_some_and(|point| point.code_point == AsciiCodePoint::OPEN_ANGLE as i32)
            && destination_points.len() >= 2
        {
            calc_escaped_string_from_node_points(
                destination_points,
                1,
                destination_points.len() - 1,
                true,
            )
        } else {
            calc_escaped_string_from_node_points(
                destination_points,
                0,
                destination_points.len(),
                true,
            )
        };

        let title = if let Some(title_state) = &data.title {
            if title_state.saturated && title_state.node_points.len() >= 2 {
                Some(calc_escaped_string_from_node_points(
                    &title_state.node_points,
                    1,
                    title_state.node_points.len() - 1,
                    false,
                ))
            } else {
                None
            }
        } else {
            None
        };

        nodes.push(Node::Definition(Definition {
            position: if parse_api.should_reserve_position() {
                calc_effective_position(token, data)
            } else {
                None
            },
            identifier,
            label,
            url: parse_api.format_url(&destination),
            title,
        }));
    }

    nodes
}
