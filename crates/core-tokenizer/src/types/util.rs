#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeInterval {
    pub start_index: usize,
    pub end_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultOfRequiredEater {
    pub valid: bool,
    pub next_index: usize,
}

pub type ResultOfOptionalEater = usize;
