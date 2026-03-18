use yozora_ast::{Node, Position};
use yozora_character::NodePoint;

use crate::engine::token::InlineToken;
use crate::phase::NodeInterval;

pub trait ParseInlinePhaseApi {
    fn should_reserve_position(&self) -> bool;

    fn calc_position(&self, interval: NodeInterval) -> Option<Position>;

    fn format_url(&self, url: &str) -> String;

    fn get_node_points(&self) -> &[NodePoint];

    fn has_definition(&self, identifier: &str) -> bool;

    fn has_footnote_definition(&self, identifier: &str) -> bool;

    fn parse_inline_tokens(&self, tokens: &[InlineToken]) -> Vec<Node>;
}

pub trait ParseInlineHook {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node>;
}
