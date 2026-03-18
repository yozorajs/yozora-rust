use yozora_ast::Node;

pub(crate) fn parse_fenced_block_token(
    _token: (),
    _position: Option<yozora_ast::Position>,
) -> Option<Node> {
    None
}
