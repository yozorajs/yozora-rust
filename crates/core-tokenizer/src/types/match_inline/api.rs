use yozora_character::NodePoint;

use crate::types::token::InlineToken;

#[allow(non_snake_case)]
pub trait MatchInlineFallbackPhaseApi {
    fn hasDefinition(&self, identifier: &str) -> bool;

    fn hasFootnoteDefinition(&self, identifier: &str) -> bool;

    fn getNodePoints(&self) -> &[NodePoint];

    fn getBlockStartIndex(&self) -> usize;

    fn getBlockEndIndex(&self) -> usize;

    fn resolveFallbackTokens(
        &self,
        tokens: &[InlineToken],
        token_start_index: usize,
        token_end_index: usize,
    ) -> Vec<InlineToken>;
}

#[allow(non_snake_case)]
pub trait MatchInlinePhaseApi {
    fn hasDefinition(&self, identifier: &str) -> bool;

    fn hasFootnoteDefinition(&self, identifier: &str) -> bool;

    fn getNodePoints(&self) -> &[NodePoint];

    fn getBlockStartIndex(&self) -> usize;

    fn getBlockEndIndex(&self) -> usize;

    fn resolveFallbackTokens(
        &self,
        tokens: &[InlineToken],
        token_start_index: usize,
        token_end_index: usize,
    ) -> Vec<InlineToken>;

    fn resolveInternalTokens(
        &self,
        higher_priority_tokens: &[InlineToken],
        start_index: usize,
        end_index: usize,
    ) -> Vec<InlineToken>;
}
