use yozora_ast::{InlineCode, Node, Text};
use yozora_core_tokenizer::{NodeInterval, ParseInlinePhaseApi};

use crate::r#match::InlineCodeToken;

pub(crate) fn parse_inline_code_tokens(
    input: &str,
    tokens: &[InlineCodeToken],
    parse_api: Option<&dyn ParseInlinePhaseApi>,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            InlineCodeToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: calc_position(parse_api, *interval),
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            InlineCodeToken::Code { span, content } => {
                nodes.push(Node::InlineCode(InlineCode {
                    position: calc_position(parse_api, *span),
                    value: normalize_code_span(&input[content.start_index..content.end_index]),
                }));
            }
        }
    }

    nodes
}

fn calc_position(
    parse_api: Option<&dyn ParseInlinePhaseApi>,
    interval: NodeInterval,
) -> Option<yozora_ast::Position> {
    parse_api.and_then(|api| api.calc_position(interval))
}

fn normalize_code_span(raw: &str) -> String {
    let collapsed = raw
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', " ");

    let is_all_space = collapsed.chars().all(|ch| ch == ' ');
    if !is_all_space
        && collapsed.len() >= 2
        && collapsed.starts_with(' ')
        && collapsed.ends_with(' ')
    {
        return collapsed[1..collapsed.len() - 1].to_string();
    }

    collapsed
}
