use yozora_ast::{Code, Node};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{
    merge_content_lines_faithfully, BlockToken, ParseBlockPhaseApi, PhrasingContentLine,
};

#[derive(Debug, Clone)]
pub struct IndentedCodeTokenData {
    pub lines: Vec<PhrasingContentLine>,
}

pub(crate) fn parse_indented_code_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<IndentedCodeTokenData>() else {
            continue;
        };

        let mut start_line_index = 0usize;
        let mut end_line_index = data.lines.len();

        while start_line_index < end_line_index {
            let line = &data.lines[start_line_index];
            if line.first_non_whitespace_index < line.end_index {
                break;
            }
            start_line_index += 1;
        }

        while start_line_index < end_line_index {
            let line = &data.lines[end_line_index - 1];
            if line.first_non_whitespace_index < line.end_index {
                break;
            }
            end_line_index -= 1;
        }

        let contents =
            merge_content_lines_faithfully(&data.lines, start_line_index, end_line_index);
        let mut value = calc_string_from_node_points(&contents, 0, contents.len(), false);
        if !value.ends_with('\n') {
            value.push('\n');
        }

        nodes.push(Node::Code(Code {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            value,
            lang: None,
            meta: None,
        }));
    }

    nodes
}
