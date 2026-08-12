use yozora_ast::{Node, Position};
use yozora_character::NodePoint;

use crate::types::token::InlineToken;
use crate::types::util::NodeInterval;
pub trait ParseInlinePhaseApi {
    fn should_reserve_position(&self) -> bool;

    fn calc_position(&self, interval: NodeInterval) -> Position;

    fn format_url(&self, url: &str) -> String;

    fn get_node_points(&self) -> &[NodePoint];

    fn has_definition(&self, identifier: &str) -> bool;

    fn has_footnote_definition(&self, identifier: &str) -> bool;

    fn parse_inline_tokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node>;
}
