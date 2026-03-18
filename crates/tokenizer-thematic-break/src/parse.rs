use yozora_ast::{Node, ThematicBreak};

use crate::r#match::ThematicBreakToken;

pub(crate) fn parse_thematic_break_token(
    _token: ThematicBreakToken,
    _position: Option<yozora_ast::Position>,
) -> Node {
    Node::ThematicBreak(ThematicBreak { position: None })
}
