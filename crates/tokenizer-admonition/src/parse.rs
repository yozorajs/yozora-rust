use std::sync::Arc;

use yozora_ast::{Admonition, Node};
use yozora_character::{calc_escaped_string_from_node_points, is_unicode_whitespace_character};
use yozora_core_tokenizer::{
    merge_and_strip_content_lines, BlockToken, ParseBlockPhaseApi, PhrasingContentLine,
};
use yozora_tokenizer_fenced_block::FencedBlockTokenData;

pub(crate) fn parse_admonition_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<FencedBlockTokenData>() else {
            continue;
        };

        let info_string = &data.info_string;
        let mut i = 0usize;
        while i < info_string.len() && !is_unicode_whitespace_character(info_string[i].code_point) {
            i += 1;
        }

        let keyword = calc_escaped_string_from_node_points(info_string, 0, i, true);

        while i < info_string.len() && is_unicode_whitespace_character(info_string[i].code_point) {
            i += 1;
        }

        let title = if i >= info_string.len() {
            Vec::new()
        } else {
            let title_lines = vec![PhrasingContentLine {
                node_points: Arc::new(info_string.clone()),
                start_index: i,
                end_index: info_string.len(),
                first_non_whitespace_index: i,
                count_of_precede_spaces: 0,
            }];
            let contents = merge_and_strip_content_lines(&title_lines, 0, title_lines.len());
            parse_api.processInlines(&contents)
        };

        let children = parse_api.parseBlockTokens(Some(&token.children));
        nodes.push(Node::Admonition(Admonition {
            position: if parse_api.shouldReservePosition() {
                token.position.clone()
            } else {
                None
            },
            keyword,
            title,
            children,
        }));
    }

    nodes
}
