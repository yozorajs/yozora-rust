use yozora_ast::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeInterval {
    pub start_index: usize,
    pub end_index: usize,
}

pub trait MatchBlockPhaseApi {
    fn register_definition_identifier(&mut self, identifier: &str);

    fn register_footnote_definition_identifier(&mut self, identifier: &str);
}

pub trait ParseBlockPhaseApi {
    fn should_reserve_position(&self) -> bool;

    fn format_url(&self, url: &str) -> String;
}

pub trait MatchInlinePhaseApi {
    fn has_definition(&self, identifier: &str) -> bool;

    fn has_footnote_definition(&self, identifier: &str) -> bool;
}

pub trait ParseInlinePhaseApi {
    fn should_reserve_position(&self) -> bool;

    fn calc_position(&self, interval: NodeInterval) -> Option<Position>;

    fn format_url(&self, url: &str) -> String;
}
