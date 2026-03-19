use yozora_ast::{Node, Position};
use yozora_character::NodePoint;

use crate::types::token::InlineToken;
use crate::types::util::NodeInterval;

#[allow(non_snake_case)]
pub trait ParseInlinePhaseApi {
    fn shouldReservePosition(&self) -> bool;

    fn calcPosition(&self, interval: NodeInterval) -> Position;

    fn formatUrl(&self, url: &str) -> String;

    fn getNodePoints(&self) -> &[NodePoint];

    fn hasDefinition(&self, identifier: &str) -> bool;

    fn hasFootnoteDefinition(&self, identifier: &str) -> bool;

    fn parseInlineTokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node>;
}
