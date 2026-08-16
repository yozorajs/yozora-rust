use yozora_character::NodePoint;
use yozora_core_tokenizer::PhrasingContentLine;

#[derive(Debug, Clone)]
pub struct LinkLabelCollectingState {
    pub saturated: bool,
    pub node_points: Vec<NodePoint>,
    pub has_non_whitespace_character: bool,
}

#[derive(Debug, Clone)]
pub struct LinkDestinationCollectingState {
    pub saturated: bool,
    pub node_points: Vec<NodePoint>,
    pub has_open_angle_bracket: bool,
    pub open_parens_count: i32,
}

#[derive(Debug, Clone)]
pub struct LinkTitleCollectingState {
    pub saturated: bool,
    pub node_points: Vec<NodePoint>,
    pub wrap_symbol: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct CollectResult<T> {
    pub next_index: isize,
    pub state: T,
}

#[derive(Debug, Clone)]
pub struct DefinitionTokenData {
    pub lines: Vec<PhrasingContentLine>,
    pub label: LinkLabelCollectingState,
    pub destination: Option<LinkDestinationCollectingState>,
    pub title: Option<LinkTitleCollectingState>,
    pub line_no_of_label: usize,
    pub line_no_of_destination: isize,
    pub line_no_of_title: isize,
    pub _label: Option<String>,
    pub _identifier: Option<String>,
}

pub const DEFINITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-definition";
