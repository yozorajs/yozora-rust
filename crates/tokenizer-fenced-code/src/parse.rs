use yozora_ast::{Code, Node};
use yozora_character::{
    calc_escaped_string_from_node_points, calc_string_from_node_points, is_whitespace_character,
};
use yozora_core_tokenizer::{merge_content_lines_faithfully, BlockToken, ParseBlockPhaseApi};
use yozora_tokenizer_fenced_block::FencedBlockTokenData;

pub(crate) fn parse_fenced_code_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<FencedBlockTokenData>() else {
            continue;
        };

        let (lang, meta) = parse_info_string(&data.info_string);
        let contents = merge_content_lines_faithfully(&data.lines, 0, data.lines.len());
        let mut value = calc_string_from_node_points(&contents, 0, contents.len(), false);
        if !value.ends_with('\n') {
            value.push('\n');
        }

        nodes.push(Node::Code(Code {
            position: if parse_api.shouldReservePosition() {
                token.position.clone()
            } else {
                None
            },
            value,
            lang,
            meta,
        }));
    }

    nodes
}

fn parse_info_string(
    info_string: &[yozora_character::NodePoint],
) -> (Option<String>, Option<String>) {
    let mut i = 0usize;
    while i < info_string.len() && !is_whitespace_character(info_string[i].code_point) {
        i += 1;
    }

    let lang = calc_escaped_string_from_node_points(info_string, 0, i, true);
    while i < info_string.len() && is_whitespace_character(info_string[i].code_point) {
        i += 1;
    }
    let meta = calc_escaped_string_from_node_points(info_string, i, info_string.len(), true);

    (
        if lang.is_empty() { None } else { Some(lang) },
        if meta.is_empty() { None } else { Some(meta) },
    )
}
