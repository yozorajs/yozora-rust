use yozora_ast::Node;
use yozora_character::NodePoint;

use crate::types::token::BlockToken;

#[derive(Debug, Clone, Copy)]
pub struct ParseBlockTokensRequest<'a> {
    pub tokens: Option<&'a [BlockToken]>,
}

pub trait ParseBlockPhaseApi {
    fn should_reserve_position(&self) -> bool;

    fn format_url(&self, url: &str) -> String;

    fn process_inlines(&self, node_points: &[NodePoint]) -> Vec<Node>;

    fn request_block_tokens<'a>(
        &self,
        tokens: Option<&'a [BlockToken]>,
    ) -> ParseBlockTokensRequest<'a> {
        ParseBlockTokensRequest { tokens }
    }
}
