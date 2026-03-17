pub type CodePoint = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodePoint {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub code_point: CodePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeInterval {
    pub start_index: usize,
    pub end_index: usize,
}
