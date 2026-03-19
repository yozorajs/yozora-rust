use yozora_ast::{Html, Node};
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{merge_content_lines_faithfully, BlockToken, ParseBlockPhaseApi};

use crate::r#match::HtmlBlockTokenData;

pub(crate) fn parse_html_block_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<HtmlBlockTokenData>() else {
            continue;
        };

        let contents = merge_content_lines_faithfully(&data.lines, 0, data.lines.len());
        let value = calc_string_from_node_points(&contents, 0, contents.len(), false);
        nodes.push(Node::Html(Html {
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
