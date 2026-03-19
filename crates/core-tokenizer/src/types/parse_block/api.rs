use yozora_ast::Node;
use yozora_character::NodePoint;

use crate::types::token::BlockToken;

#[allow(non_snake_case)]
pub trait ParseBlockPhaseApi {
    fn shouldReservePosition(&self) -> bool;

    fn formatUrl(&self, url: &str) -> String;

    fn processInlines(&self, node_points: &[NodePoint]) -> Vec<Node>;

    fn parseBlockTokens(&self, tokens: Option<&[BlockToken]>) -> Vec<Node>;
}
