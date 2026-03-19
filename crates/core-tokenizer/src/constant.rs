#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerType {
    Block,
    Inline,
}

#[allow(non_snake_case)]
pub struct TokenizerPriority;

#[allow(non_snake_case)]
impl TokenizerPriority {
    pub const ATOMIC: i32 = 10;
    pub const FENCED_BLOCK: i32 = 10;
    pub const CONTAINING_BLOCK: i32 = 10;
    pub const INTERRUPTABLE_BLOCK: i32 = 2;
    pub const IMAGES: i32 = 4;
    pub const LINKS: i32 = 3;
    pub const CONTAINING_INLINE: i32 = 2;
    pub const INTERRUPTABLE_INLINE: i32 = 2;
    pub const SOFT_INLINE: i32 = 1;
    pub const FALLBACK: i32 = -1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterType {
    Opener,
    Closer,
    Both,
    Full,
}
