use yozora_ast::Node;
use yozora_character::NodePoint;

use crate::engine::token::BlockToken;

pub trait ParseBlockPhaseApi {
    fn should_reserve_position(&self) -> bool;

    fn format_url(&self, url: &str) -> String;

    fn process_inlines(&self, node_points: &[NodePoint]) -> Vec<Node>;

    fn parse_block_tokens(&self, tokens: &[BlockToken]) -> Vec<Node>;
}

pub trait ParseBlockHook {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node>;
}
