use crate::types::phrasing_content::PhrasingContentLine;
use crate::types::token::BlockToken;

#[allow(non_snake_case)]
pub trait MatchBlockPhaseApi {
    fn extractPhrasingLines(&self, token: &BlockToken) -> Option<Vec<PhrasingContentLine>>;

    fn rollbackPhrasingLines(
        &self,
        lines: &[PhrasingContentLine],
        original_token: Option<&BlockToken>,
    ) -> Vec<BlockToken>;

    fn registerDefinitionIdentifier(&self, identifier: &str);

    fn registerFootnoteDefinitionIdentifier(&self, identifier: &str);
}
