use yozora_ast::{Node, ThematicBreak};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

pub(crate) fn parse_thematic_break_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    tokens
        .iter()
        .map(|token| {
            Node::ThematicBreak(ThematicBreak {
                position: if parse_api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
            })
        })
        .collect()
}
