use crate::types::phrasing_content::PhrasingContentLine;
use crate::types::token::BlockToken;
pub trait MatchBlockPhaseApi {
    fn extract_phrasing_lines(&self, token: &BlockToken) -> Option<Vec<PhrasingContentLine>>;

    fn rollback_phrasing_lines(
        &self,
        lines: &[PhrasingContentLine],
        original_token: Option<&BlockToken>,
    ) -> Vec<BlockToken>;

    fn register_definition_identifier(&self, identifier: &str);

    fn register_footnote_definition_identifier(&self, identifier: &str);
}
