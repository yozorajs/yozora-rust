use yozora_character::NodePoint;

use crate::types::token::InlineToken;

pub trait MatchInlinePhaseApi {
    fn has_definition(&self, identifier: &str) -> bool;

    fn has_footnote_definition(&self, identifier: &str) -> bool;

    fn get_node_points(&self) -> &[NodePoint] {
        &[]
    }

    fn get_block_start_index(&self) -> usize {
        0
    }

    fn get_block_end_index(&self) -> usize {
        0
    }

    fn resolve_fallback_tokens(
        &self,
        tokens: &[InlineToken],
        token_start_index: usize,
        token_end_index: usize,
    ) -> Vec<InlineToken> {
        let _ = (token_start_index, token_end_index);
        tokens.to_vec()
    }

    fn resolve_internal_tokens(
        &self,
        higher_priority_tokens: &[InlineToken],
        start_index: usize,
        end_index: usize,
    ) -> Vec<InlineToken> {
        let _ = (start_index, end_index);
        higher_priority_tokens.to_vec()
    }
}
