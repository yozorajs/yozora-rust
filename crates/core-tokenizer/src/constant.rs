#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerType {
    Block,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterType {
    Opener,
    Closer,
    Both,
    Full,
}
