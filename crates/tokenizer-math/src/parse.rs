use yozora_ast::{Math, Node};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{
    merge_content_lines_faithfully, BlockToken, ParseBlockPhaseApi, PhrasingContentLine,
};

#[derive(Debug, Clone)]
pub(crate) struct MathTokenData {
    pub marker_count: usize,
    pub indent: usize,
    pub lines: Vec<PhrasingContentLine>,
}

pub(crate) fn parse_math_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<MathTokenData>() else {
            continue;
        };

        let contents = merge_content_lines_faithfully(&data.lines, 0, data.lines.len());
        let mut value = calc_string_from_node_points(&contents, 0, contents.len(), false);
        if !value.ends_with('\n') {
            value.push('\n');
        }

        nodes.push(Node::Math(Math {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            value,
        }));
    }

    nodes
}
